#!/usr/bin/env python3
"""Export the Reactome graph from Neo4j into per-pass CSVs for the TypeDB load.

Neo4j is the source rather than MySQL because the TypeQL schema was derived
from the same class hierarchy the graph exposes, so labels map to entity types
and relationship types map to relations with no extra reconciliation.

Two things make this more than a dump:

  * Entities are written per *most specific* label. A Reaction node carries
    DatabaseObject/Event/ReactionLikeEvent/Reaction, but TypeDB needs the one
    concrete type, so the same hierarchy logic that built the schema picks it.

  * The n-ary relations are reassembled here, not in TypeQL. Reactome reifies
    catalysis, regulation and entity-functional-status as intermediate nodes;
    the export joins through them so each CSV row is one complete n-ary fact.
    That join is the whole point of the modelling and has to happen somewhere.

Usage: export.py [outdir]      (default: data/reactome/typedb/work)
"""

import csv
import importlib.util
import json
import os
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
NEO4J_HTTP = os.environ.get("NEO4J_HTTP", "http://localhost:7474/db/neo4j/tx/commit")
NEO4J_USER = os.environ.get("NEO4J_USER", "neo4j")
NEO4J_PASS = os.environ.get("NEO4J_PASS", "password")

_spec = importlib.util.spec_from_file_location("build_schema", HERE / "build_schema.py")
_bs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_bs)

# Per-entity attribute columns, keyed by TypeQL entity label. `db-id`,
# `display-name` and `schema-class` are on every row and added automatically.
#
# Declared on the highest type that carries the property; subtypes inherit it
# (see `entity_attrs_for`), mirroring build_schema's OWNERSHIP convention so the
# two can be read side by side. A source that is a bare property name is read as
# `n.<name>`; anything else is used as a Cypher expression verbatim.
ENTITY_ATTRS = {
    "event": [("definition", "definition"), ("release-date", "releaseDate")],
    "pathway": [("is-canonical", "isCanonical")],
    "reaction-like-event": [("is-chimeric", "isChimeric"),
                            ("systematic-name", "systematicName")],
    "physical-entity": [("definition", "definition"),
                        ("systematic-name", "systematicName")],
    "reference-entity": [("identifier", "identifier")],
    "reference-sequence": [("sequence-length", "sequenceLength")],
    "reference-isoform": [("variant-identifier", "variantIdentifier")],
    "reference-therapeutic": [("approved", "approved"), ("withdrawn", "withdrawn"),
                              ("therapeutic-type", "type")],
    "database-identifier": [("identifier", "identifier")],
    "external-ontology": [("identifier", "identifier"), ("definition", "definition")],
    "go-term": [("accession", "accession"), ("definition", "definition")],
    "go-molecular-function": [("ec-number", "ecNumber")],
    "taxon": [("tax-id", "taxId"), ("abbreviation", "abbreviation")],
    "person": [("first-name", "firstname"), ("surname", "surname")],
    # Reactome stores the edit timestamp as "2003-06-12 04:00:00"; TypeDB's
    # datetime wants the ISO separator, so it is fixed up in the export rather
    # than left to fail one row at a time in the loader.
    "instance-edit": [("edited-on", "replace(n.dateTime, ' ', 'T')"),
                      ("note", "note")],
    "summation": [("summary-text", "text")],
    "abstract-modified-residue": [("coordinate", "coordinate")],
    "release": [("release-number", "releaseNumber"), ("release-date", "releaseDate")],
}

# Multi-valued attributes. A node property that is a list cannot share the
# entity's CSV — one cell cannot hold several values — so each gets its own
# (db-id, value) file and its own pass.
MULTI_ATTRS = [
    # (typeql entity, typeql attribute, Reactome label, node property)
    ("affiliation", "affiliation-name", "Affiliation", "name"),
    # Reactome's `name` slot, one entry per class that declares it. The labels
    # are supertypes, so `MATCH (n:Event)` covers Pathway, Reaction and the
    # rest; between them these reach all 48 labels in the graph carrying
    # `name`. They are disjoint, so no object is exported twice.
    ("event", "entity-name", "Event", "name"),
    ("physical-entity", "entity-name", "PhysicalEntity", "name"),
    ("reference-entity", "entity-name", "ReferenceEntity", "name"),
    ("external-ontology", "entity-name", "ExternalOntology", "name"),
    ("go-term", "entity-name", "GO_Term", "name"),
    ("controlled-vocabulary", "entity-name", "ControlledVocabulary", "name"),
    ("taxon", "entity-name", "Taxon", "name"),
    ("functional-status-type", "entity-name", "FunctionalStatusType", "name"),
    ("reference-database", "entity-name", "ReferenceDatabase", "name"),
    ("release", "entity-name", "Release", "name"),
    ("deleted-instance", "entity-name", "DeletedInstance", "name"),
    # geneName sits on ReferenceSequence and, for 85 nodes, ReferenceMolecule.
    ("reference-sequence", "gene-name", "ReferenceSequence", "geneName"),
    ("reference-molecule", "gene-name", "ReferenceMolecule", "geneName"),
    ("update-tracker", "action-name", "UpdateTracker", "action"),
]

# Relations whose role player is itself a relation: the literature evidence for
# a catalysis or a regulation points at that fact, not at a reified stand-in for
# it. They load in their own phase after everything else, because the relation
# they reference must already exist and `rel__` passes run in glob order —
# `rel__regulation-evidence` would otherwise sort ahead of `rel__requirement__*`.
POST_BINARY = [
    ("catalyst-activity-evidence", "evidencing-reference", "evidenced-catalysis",
     "(a:CatalystActivityReference)-[r:catalystActivity]->(b:CatalystActivity)", []),
    ("regulation-evidence", "evidencing-reference", "evidenced-regulation",
     "(a:RegulationReference)-[r:regulation]->(b:Regulation)", []),
    ("regulation-go-annotation", "annotated-regulation", "regulation-biological-process",
     "(a:Regulation)-[r:goBiologicalProcess]->(b:GO_BiologicalProcess)", []),
]

# Binary relations: (typeql relation, role A, role B, cypher pattern, extra cols)
# The pattern binds $a and $b; direction is written as it exists in the graph.
BINARY = [
    ("event-containment", "containing-pathway", "contained-event",
     "(a:Pathway)-[r:hasEvent]->(b:Event)", [("ordering", "r.order")]),
    ("event-precedence", "preceding-event", "following-event",
     "(b:Event)-[r:precedingEvent]->(a:Event)", []),
    ("reaction-input", "reaction", "consumed-entity",
     "(a:ReactionLikeEvent)-[r:input]->(b:PhysicalEntity)",
     [("ordering", "r.order"), ("stoichiometry", "r.stoichiometry")]),
    ("reaction-output", "reaction", "produced-entity",
     "(a:ReactionLikeEvent)-[r:output]->(b:PhysicalEntity)",
     [("ordering", "r.order"), ("stoichiometry", "r.stoichiometry")]),
    ("required-input-component", "reaction", "required-component",
     "(a:ReactionLikeEvent)-[r:requiredInputComponent]->(b:PhysicalEntity)", []),
    ("complex-composition", "containing-complex", "component",
     "(a:Complex)-[r:hasComponent]->(b:PhysicalEntity)",
     [("ordering", "r.order"), ("stoichiometry", "r.stoichiometry")]),
    ("set-membership", "containing-set", "member",
     "(a:EntitySet)-[r:hasMember]->(b:PhysicalEntity)", [("ordering", "r.order")]),
    ("candidate-membership", "containing-set", "candidate",
     "(a:CandidateSet)-[r:hasCandidate]->(b:PhysicalEntity)", []),
    ("polymer-repetition", "containing-polymer", "repeated-unit",
     "(a:Polymer)-[r:repeatedUnit]->(b:PhysicalEntity)", []),
    ("species-assignment", "classified-thing", "species",
     "(a)-[r:species]->(b:Taxon)", []),
    ("related-species-assignment", "classified-thing", "related-species",
     "(a)-[r:relatedSpecies]->(b:Species)", []),
    ("compartment-assignment", "localised-thing", "compartment",
     "(a)-[r:compartment]->(b:Compartment)", []),
    ("included-location", "localised-thing", "location",
     "(a)-[r:includedLocation]->(b)", []),
    ("disease-annotation", "diseased-thing", "disease",
     "(a)-[r:disease]->(b)", []),
    ("go-annotation", "annotated-event", "biological-process",
     "(a:Event)-[r:goBiologicalProcess]->(b:GO_BiologicalProcess)", []),
    ("reference-assignment", "instance-entity", "reference",
     "(a:PhysicalEntity)-[r:referenceEntity]->(b:ReferenceEntity)", []),
    ("modified-residue-assignment", "modified-entity", "residue",
     "(a:EntityWithAccessionedSequence)-[r:hasModifiedResidue]->(b:AbstractModifiedResidue)", []),
    ("cross-reference", "referring-thing", "external-identifier",
     "(a)-[r:crossReference]->(b:DatabaseIdentifier)", []),
    ("database-of", "external-thing", "reference-database",
     "(a)-[r:referenceDatabase]->(b:ReferenceDatabase)", []),
    ("ontology-parenthood", "ontology-child", "ontology-parent",
     "(a)-[r:instanceOf]->(b)", []),
    ("taxonomy-parenthood", "sub-taxon", "super-taxon",
     "(a:Taxon)-[r:superTaxon]->(b:Taxon)", []),
    # inferredTo runs source -> inferred in the graph, the opposite of the
    # relational inferredFrom; the roles here restore the Reactome reading.
    ("event-inference", "inferred-event", "source-event",
     "(b:Event)-[r:inferredTo]->(a:Event)", []),
    ("entity-inference", "inferred-entity", "source-entity",
     "(b:PhysicalEntity)-[r:inferredTo]->(a:PhysicalEntity)", []),
    ("literature-citation", "citing-thing", "cited-publication",
     "(a)-[r:literatureReference]->(b:Publication)", []),
    ("summarisation", "summarised-thing", "summation",
     "(a)-[r:summation]->(b:Summation)", []),
    ("publication-authorship", "publication", "publication-author",
     "(b:Person)-[r:author]->(a:Publication)", [("ordering", "r.order")]),
    ("person-affiliation", "affiliated-person", "affiliation",
     "(a:Person)-[r:affiliation]->(b:Affiliation)", []),
    ("edit-authorship", "authored-edit", "edit-author",
     "(b:Person)-[r:author]->(a:InstanceEdit)", [("ordering", "r.order")]),
    ("functional-status-typing", "typed-status", "status-type",
     "(a:FunctionalStatus)-[r:functionalStatusType]->(b)", []),
    # Curation: the graph runs InstanceEdit -> object for most of these, but
    # internalReviewed runs the other way. Both are written to the same shape.
    ("creation", "curated-object", "edit",
     "(b:InstanceEdit)-[r:created]->(a)", []),
    # Reactome writes `created` from the edit for most classes but towards it
    # for DatabaseIdentifier and the reference sequences — 1.5M edges, over half
    # the type. Same inconsistency as internalReviewed, vastly larger.
    #
    # The WHERE is what keeps this from double-reading the 2,640 edges with an
    # InstanceEdit at BOTH ends, which otherwise match this pattern and the one
    # above, yielding two relations with the roles inverted in one of them.
    # Those edges are forward-shaped: `created` is single-valued, so the end
    # that is unique per edge owns the slot and the reused end is the creating
    # edit — and there the target is unique (2,640 of 2,640) while the source
    # repeats (2,192). Do not read this off dateTime, which says the opposite:
    # creating edits are often later bulk operations back-registering older ones.
    ("creation__reversed", "curated-object", "edit",
     "(a)-[r:created]->(b:InstanceEdit) WHERE NOT a:InstanceEdit", []),
    ("modification", "curated-object", "edit",
     "(b:InstanceEdit)-[r:modified]->(a)", []),
    ("authoring", "curated-object", "edit",
     "(b:InstanceEdit)-[r:authored]->(a)", []),
    ("review", "curated-object", "edit",
     "(b:InstanceEdit)-[r:reviewed]->(a)", []),
    ("revision", "curated-object", "edit",
     "(b:InstanceEdit)-[r:revised]->(a)", []),
    ("internal-review", "curated-object", "edit",
     "(a)-[r:internalReviewed]->(b:InstanceEdit)", []),
    ("editing", "curated-object", "edit",
     "(b:InstanceEdit)-[r:edited]->(a)", []),
    ("structure-modification", "curated-object", "edit",
     "(a)-[r:structureModified]->(b:InstanceEdit)", []),

    # --- reference sequence cross-links ---
    ("reference-gene-link", "referring-sequence", "gene-sequence",
     "(a:ReferenceSequence)-[r:referenceGene]->(b:ReferenceDNASequence)", []),
    ("reference-transcript-link", "referring-sequence", "transcript-sequence",
     "(a:ReferenceSequence)-[r:referenceTranscript]->(b:ReferenceRNASequence)", []),
    ("residue-reference-sequence", "modified-residue", "residue-sequence",
     "(a:AbstractModifiedResidue)-[r:referenceSequence]->(b:ReferenceSequence)", []),
    ("second-reference-sequence", "crosslinked-residue", "partner-sequence",
     "(a:InterChainCrosslinkedResidue)-[r:secondReferenceSequence]->(b:ReferenceSequence)", []),
    ("isoform-parenthood", "isoform", "parent-gene-product",
     "(a:ReferenceIsoform)-[r:isoformParent]->(b:ReferenceGeneProduct)", []),
    ("residue-modification", "modified-residue", "modifying-group",
     "(a:AbstractModifiedResidue)-[r:modification]->(b)", []),
    ("crosslink-equivalence", "crosslinked-residue", "equivalent-residue",
     "(a:InterChainCrosslinkedResidue)-[r:equivalentTo]->(b:InterChainCrosslinkedResidue)", []),

    # --- interactions ---
    ("interaction-participation", "interaction", "interactor",
     "(a:Interaction)-[r:interactor]->(b:ReferenceEntity)", []),

    # --- cells, markers and anatomy ---
    ("cell-marker-reference", "marked-cell", "cell-marker-ref",
     "(a:Cell)-[r:markerReference]->(b:MarkerReference)", []),
    ("marker-reference-cell", "referencing-marker", "referenced-cell",
     "(a:MarkerReference)-[r:cell]->(b:Cell)", []),
    ("marker-assignment", "marker-ref", "marker-entity",
     "(a:MarkerReference)-[r:marker]->(b:EntityWithAccessionedSequence)", []),
    ("protein-marker-assignment", "marked-cell", "protein-marker",
     "(a:Cell)-[r:proteinMarker]->(b:EntityWithAccessionedSequence)", []),
    ("rna-marker-assignment", "marked-cell", "rna-marker",
     "(a:Cell)-[r:RNAMarker]->(b:EntityWithAccessionedSequence)", []),
    ("organ-assignment", "localised-cell", "organ",
     "(a:Cell)-[r:organ]->(b:Anatomy)", []),
    ("tissue-assignment", "localised-thing", "tissue",
     "(a)-[r:tissue]->(b:Anatomy)", []),
    ("tissue-layer-assignment", "localised-cell", "tissue-layer",
     "(a:Cell)-[r:tissueLayer]->(b:Anatomy)", []),
    ("cell-type-assignment", "typed-thing", "assigned-cell-type",
     "(a)-[r:cellType]->(b:CellType)", []),

    # --- compartment topology ---
    ("compartment-membership", "part-compartment", "whole-compartment",
     "(a:GO_CellularComponent)-[r:componentOf]->(b:GO_CellularComponent)", []),
    ("compartment-part", "whole-compartment", "part-compartment",
     "(a:GO_CellularComponent)-[r:hasPart]->(b:GO_CellularComponent)", []),
    ("compartment-surrounding", "surrounded-compartment", "surrounding-compartment",
     "(a:GO_CellularComponent)-[r:surroundedBy]->(b:GO_CellularComponent)", []),
    ("go-cellular-component-assignment", "localised-thing", "cellular-component",
     "(a)-[r:goCellularComponent]->(b:GO_CellularComponent)", []),

    # --- control references ---
    ("catalyst-activity-reference-link", "referencing-reaction", "catalyst-reference",
     "(a:ReactionLikeEvent)-[r:catalystActivityReference]->(b:CatalystActivityReference)", []),
    ("regulation-reference-link", "referencing-reaction", "regulation-ref",
     "(a:ReactionLikeEvent)-[r:regulationReference]->(b:RegulationReference)", []),

    # --- review status, evidence and illustration ---
    ("review-status-assignment", "reviewed-thing", "status",
     "(a)-[r:reviewStatus]->(b:ReviewStatus)", []),
    ("previous-review-status-assignment", "reviewed-thing", "previous-status",
     "(a)-[r:previousReviewStatus]->(b:ReviewStatus)", []),
    ("evidence-type-assignment", "evidenced-thing", "evidence",
     "(a)-[r:evidenceType]->(b:EvidenceType)", []),
    ("figure-illustration", "illustrated-thing", "figure",
     "(a)-[r:figure]->(b:Figure)", []),
    ("psi-mod-assignment", "modified-thing", "psi-mod-term",
     "(a)-[r:psiMod]->(b:PsiMod)", []),
    ("structural-variant-assignment", "varied-status", "variant-term",
     "(a:FunctionalStatus)-[r:structuralVariant]->(b:SequenceOntology)", []),
    ("publication-publisher", "published-work", "publisher",
     "(a:Book)-[r:publisher]->(b:Affiliation)", []),

    # --- release and deletion bookkeeping ---
    ("release-record", "tracked-object", "tracking-release",
     "(a:UpdateTracker)-[r:release]->(b:Release)", []),
    ("update-tracking", "tracker", "updated-object",
     "(a:UpdateTracker)-[r:updatedInstance]->(b)", []),
    ("deleted-instance-record", "deletion", "deleted-thing",
     "(a:Deleted)-[r:deletedInstance]->(b:DeletedInstance)", []),
    ("replacement-instance", "deletion", "replacement",
     "(a:Deleted)-[r:replacementInstances]->(b)", []),
    ("deletion-reason", "deletion", "reason",
     "(a:Deleted)-[r:reason]->(b:DeletedControlledVocabulary)", []),

    # --- alternative event structure ---
    ("encapsulated-event", "encapsulating-pathway", "encapsulated",
     "(a:Pathway)-[r:hasEncapsulatedEvent]->(b:Pathway)", []),
    ("normal-reaction-link", "disease-reaction", "normal-reaction",
     "(a:ReactionLikeEvent)-[r:normalReaction]->(b:ReactionLikeEvent)", []),
    ("normal-pathway-link", "disease-pathway", "normal-pathway",
     "(a:Pathway)-[r:normalPathway]->(b:Pathway)", []),
    ("reverse-reaction-link", "forward-reaction", "reverse-reaction",
     "(a:ReactionLikeEvent)-[r:reverseReaction]->(b:ReactionLikeEvent)", []),
    ("entity-on-other-cell", "interacting-thing", "other-cell-entity",
     "(a)-[r:entityOnOtherCell]->(b:PhysicalEntity)", []),
    ("reaction-type-assignment", "typed-reaction", "assigned-reaction-type",
     "(a:ReactionLikeEvent)-[r:reactionType]->(b:ReactionType)", []),
]

# `regulation` rows carry the concrete subtype in a `subtype` column; each
# becomes its own pass, which is what makes requirement load as a
# positive-regulation without any extra statement.
#
# Optional roles and attributes need no such split. A blank cell is a null to
# the loader, and generate_passes wraps every column that is ever blank in a
# `try` block, so one pass takes rows whether or not they carry the optional.
SUBTYPE_COL = {"regulation": "subtype"}

# N-ary relations reassembled by joining through Reactome's reified nodes.
# N-ary relations reassembled by joining through Reactome's reified nodes.
#
# One reified node becomes exactly one relation. Where the node is referenced by
# several events, or points at several active units or statuses, those become
# several players on one role rather than several copies of the relation — the
# sharing is a fact about the node, not a reason to duplicate it. Because a CSV
# row is flat and cannot carry a role with 63 players, each relation exports as:
#
#   base   one row per node: its attributes, its single-valued roles, and the
#          FIRST player (lowest db-id) of each multi-valued role, so the
#          @card(1..) roles are satisfied the moment the relation is inserted
#   link   one row per remaining player, loaded after the base pass by matching
#          the relation on its db-id key and adding the player
#
# The base queries match the referencing event rather than OPTIONAL-matching it,
# which drops 2 CatalystActivity and 5 Regulation nodes that no event references.
# A catalysis that catalyses nothing cannot satisfy `catalysed-reaction
# @card(1..)`, and the previous loader excluded them for the same reason.
NARY = {
    "catalysis": {
        "base": """
MATCH (ca:CatalystActivity)-[:physicalEntity]->(cat)
OPTIONAL MATCH (rle:ReactionLikeEvent)-[:catalystActivity]->(ca)
OPTIONAL MATCH (ca)-[:activity]->(act)
WITH ca, cat, act, min(rle.dbId) AS catalysed_reaction
OPTIONAL MATCH (ca)-[:activeUnit]->(u)
WITH ca, cat, act, catalysed_reaction, min(u.dbId) AS active_unit
RETURN ca.dbId AS db_id, ca.displayName AS display_name,
       ca.schemaClass AS schema_class, catalysed_reaction,
       cat.dbId AS catalyst, act.dbId AS catalytic_activity, active_unit""",
        "links": {
            "catalysed_reaction": """
MATCH (rle:ReactionLikeEvent)-[:catalystActivity]->(ca:CatalystActivity)
WITH ca, min(rle.dbId) AS first, collect(DISTINCT rle.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN ca.dbId AS db_id, v AS catalysed_reaction""",
            "active_unit": """
MATCH (ca:CatalystActivity)-[:activeUnit]->(u)
WITH ca, min(u.dbId) AS first, collect(DISTINCT u.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN ca.dbId AS db_id, v AS active_unit""",
        },
    },
    # The regulation subtype comes from the reified node's own label, which is
    # exactly the distinction SQL can only reach through the class table.
    "regulation": {
        "base": """
MATCH (reg:Regulation)-[:regulator]->(who)
OPTIONAL MATCH (rle:ReactionLikeEvent)-[:regulatedBy]->(reg)
OPTIONAL MATCH (reg)-[:activity]->(act)
WITH reg, who, act, min(rle.dbId) AS regulated_event
OPTIONAL MATCH (reg)-[:activeUnit]->(u)
WITH reg, who, act, regulated_event, min(u.dbId) AS regulation_active_unit
RETURN reg.dbId AS db_id, reg.displayName AS display_name,
       reg.schemaClass AS schema_class, reg.stId AS st_id, regulated_event,
       who.dbId AS regulator, act.dbId AS regulatory_activity,
       regulation_active_unit,
       CASE
         WHEN reg:Requirement THEN 'Requirement'
         WHEN reg:PositiveGeneExpressionRegulation THEN 'PositiveGeneExpressionRegulation'
         WHEN reg:NegativeGeneExpressionRegulation THEN 'NegativeGeneExpressionRegulation'
         WHEN reg:PositiveRegulation THEN 'PositiveRegulation'
         WHEN reg:NegativeRegulation THEN 'NegativeRegulation'
       END AS subtype""",
        "links": {
            "regulated_event": """
MATCH (rle:ReactionLikeEvent)-[:regulatedBy]->(reg:Regulation)
WITH reg, min(rle.dbId) AS first, collect(DISTINCT rle.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN reg.dbId AS db_id, v AS regulated_event""",
            "regulation_active_unit": """
MATCH (reg:Regulation)-[:activeUnit]->(u)
WITH reg, min(u.dbId) AS first, collect(DISTINCT u.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN reg.dbId AS db_id, v AS regulation_active_unit""",
        },
    },
    "entity-functional-status": {
        "base": """
MATCH (rle:ReactionLikeEvent)-[:entityFunctionalStatus]->(efs:EntityFunctionalStatus)
MATCH (efs)-[:diseaseEntity]->(de)
MATCH (efs)-[:functionalStatus]->(fs)
OPTIONAL MATCH (efs)-[:normalEntity]->(ne)
WITH efs, de, ne, min(rle.dbId) AS affected_event, min(fs.dbId) AS functional_status
RETURN efs.dbId AS db_id, efs.displayName AS display_name,
       efs.schemaClass AS schema_class, affected_event, de.dbId AS disease_entity,
       ne.dbId AS normal_entity, functional_status""",
        "links": {
            "affected_event": """
MATCH (rle:ReactionLikeEvent)-[:entityFunctionalStatus]->(efs:EntityFunctionalStatus)
WITH efs, min(rle.dbId) AS first, collect(DISTINCT rle.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN efs.dbId AS db_id, v AS affected_event""",
            "functional_status": """
MATCH (efs:EntityFunctionalStatus)-[:functionalStatus]->(fs)
WHERE (:ReactionLikeEvent)-[:entityFunctionalStatus]->(efs)
WITH efs, min(fs.dbId) AS first, collect(DISTINCT fs.dbId) AS ids
UNWIND [x IN ids WHERE x <> first] AS v
RETURN efs.dbId AS db_id, v AS functional_status""",
        },
    },
    # Already one-to-one in the graph: every NegativePrecedingEvent has exactly
    # one preceding event, one following event and at most one reason.
    "negative-precedence": {
        "base": """
MATCH (ev:Event)-[:negativePrecedingEvent]->(npe:NegativePrecedingEvent)
MATCH (npe)-[:precedingEvent]->(prev)
OPTIONAL MATCH (npe)-[:reason]->(why)
RETURN npe.dbId AS db_id, npe.displayName AS display_name,
       npe.schemaClass AS schema_class, prev.dbId AS excluded_preceding_event,
       ev.dbId AS following_event, why.dbId AS exclusion_reason""",
        "links": {},
    },
}


def var(label: str) -> str:
    """Attribute/role label to a loader variable name. Hyphens are valid in
    type labels but not in `$variables`, so the CSV header uses underscores."""
    return label.replace("-", "_")


def cypher(query: str) -> list[list[str]]:
    """Run a read query and return header + rows.

    Neo4j's HTTP endpoint is used rather than cypher-shell because
    `--format plain` does not quote values containing commas: two Taxon names
    ("dsDNA viruses, no RNA stage") split across columns and were rejected by
    the loader. JSON has no such ambiguity.
    """
    body = json.dumps({"statements": [{"statement": query}]})
    # encoding is explicit: `text=True` alone decodes with the locale codec,
    # which on Windows is cp1252 and dies on the first byte Reactome's free text
    # uses outside it — the reader thread raises, stdout comes back None, and
    # the failure surfaces as a JSON parse error a long way from the cause.
    out = subprocess.run(
        ["curl", "-sS", "-u", f"{NEO4J_USER}:{NEO4J_PASS}",
         "-H", "Content-Type: application/json", "-d", body, NEO4J_HTTP],
        capture_output=True, text=True, encoding="utf-8",
    )
    if out.returncode != 0:
        raise SystemExit(f"curl failed: {out.stderr}")
    payload = json.loads(out.stdout)
    if payload.get("errors"):
        raise SystemExit(f"cypher error: {payload['errors']}\nquery:\n{query}")
    result = payload["results"][0]
    rows = [result["columns"]]
    for entry in result["data"]:
        rows.append(["" if v is None else str(v) for v in entry["row"]])
    return rows


def concrete_entity_types() -> dict[str, str]:
    """TypeQL entity label -> Reactome label, for every type that can be a
    node's most specific label."""
    parent = _bs.read_hierarchy()
    out = {}
    for label in parent:
        if label in _bs.AS_RELATIONS or label in _bs.MIXINS or label in _bs.COEXTENSIVE_LOSERS:
            continue
        out[_bs.kebab(label)] = label
    return out


def entity_attrs_for(reactome: str, parent: dict[str, str | None]) -> list[tuple[str, str]]:
    """This type's ENTITY_ATTRS columns plus every ancestor's.

    Reactome declares a slot once on the class that introduces it, so
    `definition` on Event means every Pathway and Reaction has one too. Walking
    the chain lets ENTITY_ATTRS say that once instead of repeating it for all 23
    concrete types that inherit it. Nearest declaration wins on a clash.
    """
    cols: list[tuple[str, str]] = []
    seen: set[str] = set()
    label: str | None = reactome
    while label:
        tql = _bs.kebab(label)
        for dst, src in ENTITY_ATTRS.get(tql, []):
            if dst not in seen:
                seen.add(dst)
                cols.append((dst, src))
        label = parent.get(label)
    return cols


def export_entities(outdir: pathlib.Path, only: set[str] | None) -> None:
    """One CSV per concrete entity type, keyed on db-id.

    A node is written to the pass for its most specific label only. The
    hierarchy is expressed in the schema, so writing a Reaction into both
    `reaction` and `event` would insert it twice under two @key values.
    """
    types = concrete_entity_types()
    ancestors_of = _bs.read_hierarchy()
    for tql_label, reactome in sorted(types.items()):
        if only and tql_label not in only:
            continue
        extra = entity_attrs_for(reactome, ancestors_of)
        # "most specific" = carries this label and no label that is a subtype of it
        subtypes = [l for l, p in ancestors_of.items() if p == reactome]
        guard = "".join(f" AND NOT n:{s}" for s in subtypes)
        # Fully qualified here rather than fixed up at join time: a source that
        # is an expression rather than a property name must not have `n.`
        # prepended to it.
        cols = ["n.dbId AS db_id", "n.displayName AS display_name",
                "n.schemaClass AS schema_class", "n.stId AS st_id",
                "n.oldStId AS old_st_id"]
        cols += [f"{src if not src.isidentifier() else 'n.' + src} AS {var(dst)}"
                 for dst, src in extra]
        q = f"MATCH (n:{reactome}) WHERE true{guard} RETURN " + ", ".join(cols)
        rows = cypher(q)
        path = outdir / f"entity__{tql_label}.csv"
        with path.open("w", newline="", encoding="utf-8") as fh:
            csv.writer(fh).writerows(rows)
        print(f"  {tql_label:<38} {max(len(rows) - 1, 0):>9} rows")


def write_relation(outdir: pathlib.Path, name: str, rows: list[list[str]]) -> None:
    """Write one CSV per subtype (or a single CSV when the relation has none).

    Optional columns stay in the file with blank cells; the pass handles them
    with `try` blocks. Only the concrete type has to be fixed per pass, since
    it is the one thing an insert statement cannot make conditional.
    """
    if len(rows) < 2:
        print(f"  {name:<38} {'0':>9} rows")
        return
    header, data = rows[0], rows[1:]
    idx = {c: i for i, c in enumerate(header)}
    sub = SUBTYPE_COL.get(name)
    groups: dict[str | None, list[list[str]]] = {}
    for row in data:
        groups.setdefault(row[idx[sub]] if sub else None, []).append(row)
    cols = [c for c in header if c != sub]
    keep = [idx[c] for c in cols]
    for subtype, rs in sorted(groups.items(), key=lambda kv: str(kv[0])):
        stem = _bs.kebab(subtype) if subtype else name
        path = outdir / f"rel__{stem}.csv"
        with path.open("w", newline="", encoding="utf-8") as fh:
            w = csv.writer(fh)
            w.writerow(cols)
            w.writerows([[r[i] for i in keep] for r in rs])
        print(f"  {path.stem:<38} {len(rs):>9} rows")


def export_multi(outdir: pathlib.Path, only: set[str] | None) -> None:
    for entity, attr, label, prop in MULTI_ATTRS:
        if only and attr not in only and entity not in only:
            continue
        # The same Reactome slot is a list on most labels and a bare string on a
        # few (the importer flattens single-valued ones: Compartment, DBInfo,
        # DeletedInstance and the three GO_ classes). UNWIND throws on a
        # non-list, so wrap the scalar case before unwinding.
        # Blank values are dropped: 65 Reactome name lists carry a trailing ""
        # (e.g. ["RHOG", "RhoG", ""]). An empty string is not a name, and the
        # loader rejects it as a null in a non-optional column, which fails the
        # whole load's exit status over a data-entry artifact.
        q = (f"MATCH (n:{label}) WHERE n.{prop} IS NOT NULL "
             f"UNWIND (CASE WHEN valueType(n.{prop}) STARTS WITH 'LIST' "
             f"THEN n.{prop} ELSE [n.{prop}] END) AS v "
             f"WITH n, v WHERE v IS NOT NULL AND v <> '' "
             f"RETURN n.dbId AS db_id, v AS {var(attr)}")
        rows = cypher(q)
        path = outdir / f"attr__{entity}__{attr}.csv"
        with path.open("w", newline="", encoding="utf-8") as fh:
            csv.writer(fh).writerows(rows)
        print(f"  {path.stem:<38} {max(len(rows) - 1, 0):>9} rows")


def export_binary(outdir: pathlib.Path, only: set[str] | None) -> None:
    for name, role_a, role_b, pattern, extra in BINARY:
        if only and name not in only:
            continue
        cols = [f"a.dbId AS {var(role_a)}", f"b.dbId AS {var(role_b)}"]
        cols += [f"{src} AS {var(dst)}" for dst, src in extra]
        q = f"MATCH {pattern} RETURN " + ", ".join(cols)
        write_relation(outdir, name, cypher(q))


def export_post_binary(outdir: pathlib.Path, only: set[str] | None) -> None:
    for name, role_a, role_b, pattern, extra in POST_BINARY:
        if only and name not in only:
            continue
        cols = [f"a.dbId AS {var(role_a)}", f"b.dbId AS {var(role_b)}"]
        cols += [f"{src} AS {var(dst)}" for dst, src in extra]
        rows = cypher(f"MATCH {pattern} RETURN " + ", ".join(cols))
        path = outdir / f"post__{name}.csv"
        with path.open("w", newline="", encoding="utf-8") as fh:
            csv.writer(fh).writerows(rows)
        print(f"  {path.stem:<38} {max(len(rows) - 1, 0):>9} rows")


def export_nary(outdir: pathlib.Path, only: set[str] | None) -> None:
    for name, spec in sorted(NARY.items()):
        if only and name not in only:
            continue
        write_relation(outdir, name, cypher(spec["base"].strip()))
        # One file per multi-valued role, holding every player after the first.
        # Loaded after the base pass, which is what puts the relation there to
        # be matched on its db-id.
        for role, query in sorted(spec["links"].items()):
            rows = cypher(query.strip())
            path = outdir / f"link__{name}__{role.replace('_', '-')}.csv"
            with path.open("w", newline="", encoding="utf-8") as fh:
                csv.writer(fh).writerows(rows)
            print(f"  {path.stem:<38} {max(len(rows) - 1, 0):>9} rows")


def audit_schema_coverage() -> list[str]:
    """Attributes and roles the schema declares that nothing here exports.

    A declared-but-never-exported type loads as empty, and nothing downstream
    notices: the loader's zero-reject check passes (a pass that never runs
    rejects nothing) and queries return 0, which reads as a fact about the data
    rather than a gap in it. That is how `affiliation-name`, `entity-name`,
    `gene-name` and the two active-unit roles all came to be empty. This
    compares the two sides so the next one is caught at export time.

    Reported, not fatal: a type may be deliberately unpopulated, and failing the
    export would make that impossible to express.
    """
    # Attributes the schema says something owns, minus the ones every entity
    # CSV carries as fixed columns.
    declared_attrs = {
        owned.split()[0]
        for owned_list in _bs.OWNERSHIP.values() for owned in owned_list
    } - {"db-id", "display-name", "schema-class", "st-id", "old-st-id"}
    exported_attrs = {a for _, a, _, _ in MULTI_ATTRS}
    exported_attrs |= {dst for cols in ENTITY_ATTRS.values() for dst, _ in cols}
    # Attributes owned by a relation ride along on that relation's CSV.
    exported_attrs |= {dst for *_, extra in BINARY for dst, _ in extra}

    # Roles each relation declares itself, parsed from the schema source. Roles
    # inherited from an abstract parent are deliberately not resolved: this
    # compares in one direction only (declared but not exported), so
    # under-declaring can never produce a false positive.
    declared_roles, relation = {}, None
    for line in _bs.RELATIONS.splitlines():
        m = re.match(r"\s*relation ([\w-]+)", line)
        if m:
            relation = m.group(1)
            declared_roles.setdefault(relation, set())
            continue
        m = re.match(r"\s*relates ([\w-]+)", line)
        if m and relation:
            declared_roles[relation].add(m.group(1))

    exported_roles = {}
    for name, role_a, role_b, _, _ in BINARY:
        exported_roles.setdefault(name, set()).update({role_a, role_b})
    for name, spec in NARY.items():
        queries = [spec["base"], *spec["links"].values()]
        exported_roles.setdefault(name, set()).update(
            c.replace("_", "-")
            for q in queries for c in re.findall(r"AS (\w+)", q))
    gaps = [f"attribute {a}" for a in sorted(declared_attrs - exported_attrs)]
    # Only relations the exporter emits at all are checked. An abstract parent
    # with no entry of its own (reaction-participation, composition, curation)
    # is populated entirely through its subtypes, which are checked separately.
    for name in sorted(declared_roles):
        if name not in exported_roles:
            continue
        for role in sorted(declared_roles[name] - exported_roles[name]):
            gaps.append(f"role {name}:{role}")
    return gaps


def audit_graph_coverage() -> list[str]:
    """Graph edges no pattern in this file reads.

    The schema-side audit compares the schema against the exporter, so it is
    blind to anything neither side knows about: a relationship type with no
    TypeQL relation at all looks consistent to it. It is also blind to partial
    coverage, where a type is read from one source label while edges from
    another are dropped — which is how `catalystActivity` looked mapped while
    the 1,140 edges from CatalystActivityReference went missing.

    So this counts, per relationship type, the edges in the graph against the
    edges this file's patterns actually match, and reports the shortfall.
    Requires Neo4j, so it runs at export time rather than in the linter.
    """
    patterns: dict[str, list[str]] = {}
    # The overlap check below counts these separately. A binary pattern inserts
    # exactly one relation per edge it matches, so two of them matching the same
    # edge is a duplicate; the n-ary patterns below deliberately revisit an edge
    # type (see there) and cannot be read the same way.
    binary: dict[str, list[str]] = {}
    for *_, pattern, _ in [*BINARY, *POST_BINARY]:
        for t in re.findall(r"\[r?:(\w+)\]", pattern):
            q = f"MATCH {pattern} RETURN count(r) AS c"
            patterns.setdefault(t, []).append(q)
            binary.setdefault(t, []).append(q)
    # Counted for coverage only, never for overlap. Two reasons they double-
    # count: a base and its link query walk the same edge type on purpose (the
    # base takes min(dbId) for a role and the link takes the rest), and the
    # fragments below are single lines lifted out of a multi-line query, so one
    # that leans on a label bound earlier — `(ca)-[:activity]->(act)`, where ca
    # was matched as a CatalystActivity two lines up — counts the whole graph
    # rather than its own scope.
    for spec in NARY.values():
        for q in [spec["base"], *spec["links"].values()]:
            for frag in re.findall(r"(?:OPTIONAL )?MATCH (\([^\n]*?\[:(\w+)\][^\n]*?\))", q):
                whole, t = frag
                patterns.setdefault(t, []).append(
                    f"MATCH {whole.replace('[:', '[r:', 1)} RETURN count(r) AS c")

    rows = cypher("CALL db.relationshipTypes() YIELD relationshipType "
                  "RETURN relationshipType ORDER BY relationshipType")
    gaps = []
    for (rel_type,) in rows[1:]:
        total = int(cypher(f"MATCH ()-[r:`{rel_type}`]->() RETURN count(r) AS c")[1][0])
        if not total:
            continue
        seen = 0
        for q in patterns.get(rel_type, []):
            try:
                seen += int(cypher(q)[1][0])
            except Exception:
                pass
        if seen < total:
            gaps.append(f"{rel_type}: {total - seen} of {total} edges unread")
            continue
        dup = 0
        for q in binary.get(rel_type, []):
            try:
                dup += int(cypher(q)[1][0])
            except Exception:
                pass
        if dup > total:
            # Two patterns matching the same edge is as wrong as none matching,
            # and worse to spot: it loads a duplicate relation rather than
            # dropping one. `created` is written in both directions, so the
            # pattern for each matched every edge with an InstanceEdit at both
            # ends, inverting the roles on one copy. A shortfall-only audit
            # cannot see that — seen exceeded total and the check read as pass.
            gaps.append(f"{rel_type}: {dup - total} edges read twice (of "
                        f"{total}) — binary patterns overlap, loading duplicates")
    return gaps


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only = None
    for a in sys.argv[1:]:
        if a.startswith("--only="):
            only = set(a.split("=", 1)[1].split(","))
    outdir = pathlib.Path(args[0]) if args else HERE / "work"
    outdir.mkdir(parents=True, exist_ok=True)
    # A full export owns the directory: a relation renamed or dropped here would
    # otherwise leave its CSV from the previous run behind, and the loader runs
    # every CSV it finds, mixing a stale pass into a fresh load. Cleared only
    # for a full run; --only is a partial export and must leave the rest alone.
    if not only:
        for stale in outdir.glob("*.csv"):
            stale.unlink()
    print(f"exporting into {outdir}" + (f" (only: {sorted(only)})" if only else ""))
    print("entities:")
    export_entities(outdir, only)
    print("multi-valued attributes:")
    export_multi(outdir, only)
    print("binary relations:")
    export_binary(outdir, only)
    print("n-ary relations:")
    export_nary(outdir, only)
    print("relations referencing relations:")
    export_post_binary(outdir, only)

    if not only:
        gaps = audit_schema_coverage()
        if gaps:
            print("\ndeclared in the schema but exported by nothing "
                  "(these will load as empty):")
            for gap in gaps:
                print(f"  {gap}")
        else:
            print("\nschema coverage: every declared attribute and role is exported")

        edge_gaps = audit_graph_coverage()
        if edge_gaps:
            print("\ngraph edges no pattern here reads (these are absent from TypeDB):")
            for gap in edge_gaps:
                print(f"  {gap}")
        else:
            print("graph coverage: every relationship type is fully read")


if __name__ == "__main__":
    main()
