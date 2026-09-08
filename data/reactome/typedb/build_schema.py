#!/usr/bin/env python3
"""Build the TypeQL schema for Reactome (TypeDB 3.12).

The entity hierarchy is generated from the class hierarchy observed in the
Reactome graph release (data/reactome/neo4j/schema.txt), so it tracks the real
data rather than a hand-copied reading of the docs. The relations are designed
by hand, because that is where TypeQL differs from the other two models and a
mechanical translation would throw the difference away:

  * Catalysis is a ternary relation. Reactome reifies it as a CatalystActivity
    row/node carrying a physical entity and a GO molecular function, which the
    reaction then points at — two hops in SQL and Cypher. A single
    CatalystActivity is reused by up to 63 reactions in this release, so the
    honest decomposition is one ternary fact per (catalyst, activity, reaction).

  * Regulation is a relation hierarchy. positive-regulation and its subtypes —
    including requirement, whose name gives no hint — are subtypes of one
    abstract regulation, so "regulated positively" is a single type query
    instead of the class-table join SQL needs.

  * Complex components and set members share one abstract composition relation,
    so walking a complex's parts is one recursive traversal rather than a union
    over two link tables.

  * entity-functional-status is genuinely 4-ary (event, disease entity, normal
    entity, status) and is modelled as such rather than as a hub node.

Ordering that the relational model keeps in `<attr>_rank` columns, and the
graph keeps in an `order` relationship property, lives here as an `ordering`
attribute owned by the relation.

Usage: build_schema.py [out.tql]
"""

import pathlib
import re
import sys

HIERARCHY_SRC = pathlib.Path(__file__).resolve().parents[1] / "neo4j" / "schema.txt"
# Bookkeeping labels that are not part of the domain hierarchy.
MIXINS = {"Trackable", "Deletable"}


def kebab(name: str) -> str:
    """Reactome's CamelCase class names to TypeQL's kebab-case labels."""
    name = name.replace("_", "-")
    name = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "-", name)
    return name.lower()


def read_hierarchy() -> dict[str, str | None]:
    """Direct-parent per label, derived from the 'is a' lines of the graph schema."""
    ancestors: dict[str, list[str]] = {}
    for line in HIERARCHY_SRC.read_text(encoding="utf8").splitlines():
        m = re.match(r"^(\w+) is a (.+)$", line)
        if m:
            ancestors[m.group(1)] = [x.strip() for x in m.group(2).split(",")]
    labels = sorted(set(ancestors) | {a for v in ancestors.values() for a in v})
    depth = {l: len(ancestors.get(l, [])) for l in labels}
    parent: dict[str, str | None] = {}
    for label in labels:
        # Some label pairs are co-extensive in this release — every node
        # carrying one carries the other — so containment cannot say which is
        # the subtype (Interaction/UndirectedInteraction, ReactionType/
        # DrugActionType, ModifiedNucleotide/TranscriptionalModification).
        # Treat those as siblings under their nearest shared ancestor rather
        # than inventing a direction the data does not support.
        real = [
            a for a in ancestors.get(label, [])
            if a not in MIXINS and label not in ancestors.get(a, [])
        ]
        parent[label] = max(real, key=lambda a: depth.get(a, 0)) if real else None
    return parent


# Attributes, with the value type each carries.
#
# `id` and `name` are abstract supertypes over the concrete attributes rather
# than attributes anything owns directly. Reactome spreads both concepts over
# many slots — six kinds of identifier, six kinds of name — and grouping them
# makes `$x has name $n` one polymorphic lookup over all of them instead of a
# query that has to enumerate each. Ownership is unchanged: owners still own the
# concrete subtypes, so no `owns` declaration moves.
#
# `name` and `id` each declare their value type once and their subtypes inherit
# it, so the schema itself guarantees every name and every identifier is a
# string. A subtype may not redeclare a value type it already inherits (SVL39),
# which is why the subtypes below are bare.
#
# db-id stays outside the `id` hierarchy deliberately. It is the store's own
# surrogate key, held by every object, so including it would make `$x has id $i`
# match everything and reduce "objects carrying an identifier" to "all objects".
# Outside it, `id` means an externally meaningful identifier, which is the thing
# worth asking about.
ATTRIBUTES = """
attribute db-id, value integer;

attribute id @abstract, value string;
attribute st-id sub id;
attribute old-st-id sub id;
attribute identifier sub id;
attribute accession sub id;
attribute tax-id sub id;
attribute variant-identifier sub id;

attribute name @abstract, value string;
attribute display-name sub name;
attribute entity-name sub name;
attribute gene-name sub name;
attribute systematic-name sub name;
attribute first-name sub name;
attribute surname sub name;
attribute affiliation-name sub name;

attribute schema-class, value string;
attribute definition, value string;
attribute ec-number, value string;
attribute abbreviation, value string;
attribute summary-text, value string;
attribute note, value string;
attribute release-number, value integer;
attribute release-date, value date;
attribute edited-on, value datetime;
attribute coordinate, value integer;
attribute sequence-length, value integer;
attribute approved, value boolean;
attribute withdrawn, value boolean;
attribute is-canonical, value boolean;
attribute is-chimeric, value boolean;
attribute therapeutic-type, value string;
# Deliberately outside the `name` hierarchy: this names the action an update
# tracker performed, not an object. Under `name`, "objects whose name contains
# X" would start matching update trackers by their verb.
attribute action-name, value string;
# Ordering and stoichiometry sit on the relation, not on either endpoint:
# they are facts about the participation, not about the participant.
attribute ordering, value integer;
attribute stoichiometry, value integer;
"""

# Which entity types own which attributes. Declared on the highest type that
# has them so every subtype inherits — the same polymorphism the queries use.
#
# `entity-name` is Reactome's `name` slot — the curated names and synonyms an
# object goes by, as distinct from the derived single `display-name`. It is
# owned by exactly the types whose Reactome labels carry the slot in the graph
# the loader reads (48 labels, reached by the ten owners below). Declaring it on
# database-object instead would be shorter but would permit a name on a person
# or an instance-edit, which Reactome never records.
OWNERSHIP = {
    "database-object": ["db-id @key", "display-name", "schema-class",
                        "st-id @card(0..1)", "old-st-id @card(0..1)"],
    # Events carry their own releaseDate — the release that first published
    # them — separately from the release-* entity's own date.
    "event": ["entity-name @card(0..)", "definition @card(0..1)",
              "release-date @card(0..1)"],
    "pathway": ["is-canonical @card(0..1)"],
    "reaction-like-event": ["is-chimeric @card(0..1)",
                            "systematic-name @card(0..1)"],
    "physical-entity": ["entity-name @card(0..)", "definition @card(0..1)",
                        "systematic-name @card(0..1)"],
    "reference-entity": ["identifier @card(0..1)", "entity-name @card(0..)"],
    "reference-sequence": ["gene-name @card(0..)", "sequence-length @card(0..1)"],
    # 85 ReferenceMolecules carry a geneName in the graph despite sitting
    # outside ReferenceSequence. Odd for a small molecule, but real, and
    # omitting it would leave TypeDB 85 gene names short of the other stores.
    "reference-molecule": ["gene-name @card(0..)"],
    "reference-isoform": ["variant-identifier @card(0..1)"],
    "reference-therapeutic": ["approved @card(0..1)", "withdrawn @card(0..1)",
                              "therapeutic-type @card(0..1)"],
    "go-term": ["accession @card(0..1)", "definition @card(0..1)",
                "entity-name @card(0..)"],
    "go-molecular-function": ["ec-number @card(0..)"],
    "external-ontology": ["identifier @card(0..1)", "definition @card(0..1)",
                          "entity-name @card(0..)"],
    "controlled-vocabulary": ["entity-name @card(0..)"],
    # The whole point of a database-identifier is the identifier it pairs with
    # a reference database, so it owns one directly.
    "database-identifier": ["identifier @card(0..1)"],
    "functional-status-type": ["entity-name @card(0..)"],
    "reference-database": ["entity-name @card(0..)"],
    "deleted-instance": ["entity-name @card(0..)"],
    "taxon": ["tax-id @card(0..1)", "abbreviation @card(0..1)",
              "entity-name @card(0..)"],
    "person": ["first-name @card(0..1)", "surname @card(0..1)"],
    "affiliation": ["affiliation-name @card(0..)"],
    "instance-edit": ["edited-on @card(0..1)", "note @card(0..1)"],
    "summation": ["summary-text @card(0..1)"],
    "abstract-modified-residue": ["coordinate @card(0..1)"],
    "update-tracker": ["action-name @card(0..)"],
    "release": ["release-number @card(0..1)", "release-date @card(0..1)",
                 "entity-name @card(0..)"],
}

RELATIONS = """
# ---------------------------------------------------------------------------
# Event structure
# ---------------------------------------------------------------------------

# A pathway contains events in a curated order; `ordering` is the rank the
# relational model keeps in Pathway_2_hasEvent.hasEvent_rank.
relation event-containment,
  relates containing-pathway,
  relates contained-event,
  owns ordering @card(0..1);

# Ordering between events, and the curated statement that one event does NOT
# precede another (Reactome records the reason for the exclusion).
relation event-precedence,
  relates preceding-event,
  relates following-event;

relation negative-precedence,
  owns db-id @key,
  owns display-name,
  owns schema-class,
  relates excluded-preceding-event,
  relates following-event,
  relates exclusion-reason @card(0..1);

# Reaction participants. The abstract supertype lets a query ask for any
# participation without naming the direction.
relation reaction-participation @abstract,
  relates reaction,
  relates participant,
  owns ordering @card(0..1),
  owns stoichiometry @card(0..1);

relation reaction-input sub reaction-participation,
  relates consumed-entity as participant;

relation reaction-output sub reaction-participation,
  relates produced-entity as participant;

relation required-input-component sub reaction-participation,
  relates required-component as participant;

# ---------------------------------------------------------------------------
# Catalysis — ternary
# ---------------------------------------------------------------------------
# Reactome reifies this as a CatalystActivity holding a physical entity and a
# GO molecular function, which reactions then reference. Because one such
# activity is shared by many reactions, the fact being stated is really
# three-way: this entity, performing this molecular function, catalyses this
# reaction. An active unit narrows which part of the catalyst is responsible.
# One relation per CatalystActivity node, holding every reaction that node is
# referenced by — 63 at the most. Reactome shares one activity across reactions
# rather than repeating it, and that sharing is a fact about the activity, not a
# reason to mint a separate relation per reaction. `db-id` is the node's own, so
# it keys the relation.
relation catalysis,
  owns db-id @key,
  owns display-name,
  owns schema-class,
  plays catalyst-activity-evidence:evidenced-catalysis,
  relates catalysed-reaction @card(0..),
  relates catalyst,
  relates catalytic-activity @card(0..1),
  relates active-unit @card(0..);

# ---------------------------------------------------------------------------
# Regulation — a relation hierarchy
# ---------------------------------------------------------------------------
# Subtyping carries what the relational model can only express by joining the
# PositiveRegulation table: `requirement` is a positive regulation, and its
# name says nothing about that.
relation regulation @abstract,
  owns db-id @key,
  owns display-name,
  owns schema-class,
  owns st-id @card(0..1),
  plays regulation-evidence:evidenced-regulation,
  plays regulation-go-annotation:annotated-regulation,
  relates regulated-event @card(0..),
  relates regulator,
  relates regulatory-activity @card(0..1),
  relates regulation-active-unit @card(0..);

relation positive-regulation sub regulation;
relation requirement sub positive-regulation;
relation positive-gene-expression-regulation sub positive-regulation;
relation negative-regulation sub regulation;
relation negative-gene-expression-regulation sub negative-regulation;

# ---------------------------------------------------------------------------
# Composition — one abstract relation over complexes, sets and polymers
# ---------------------------------------------------------------------------
# SQL needs Complex_2_hasComponent UNION EntitySet_2_hasMember to walk a
# structure; here both are `composition`, so one traversal covers them.
relation composition @abstract,
  relates whole,
  relates part,
  owns ordering @card(0..1),
  owns stoichiometry @card(0..1);

relation complex-composition sub composition,
  relates containing-complex as whole,
  relates component as part;

relation set-membership sub composition,
  relates containing-set as whole,
  relates member as part;

relation candidate-membership sub set-membership,
  relates candidate as member;

relation polymer-repetition sub composition,
  relates containing-polymer as whole,
  relates repeated-unit as part;

# ---------------------------------------------------------------------------
# Annotation
# ---------------------------------------------------------------------------
relation species-assignment,
  relates classified-thing,
  relates species;

relation related-species-assignment,
  relates classified-thing,
  relates related-species;

relation compartment-assignment,
  relates localised-thing,
  relates compartment;

relation included-location,
  relates localised-thing,
  relates location;

relation disease-annotation,
  relates diseased-thing,
  relates disease;

relation go-annotation,
  relates annotated-event,
  relates biological-process;

# A protein/molecule instance and the reference entity it is an instance of.
relation reference-assignment,
  relates instance-entity,
  relates reference;

relation modified-residue-assignment,
  relates modified-entity,
  relates residue;

relation cross-reference,
  relates referring-thing,
  relates external-identifier;

relation database-of,
  relates external-thing,
  relates reference-database;

# Ontology parenthood, used by GO and the external ontologies; recursive.
relation ontology-parenthood,
  relates ontology-child,
  relates ontology-parent;

relation taxonomy-parenthood,
  relates sub-taxon,
  relates super-taxon;

# ---------------------------------------------------------------------------
# Inference between species
# ---------------------------------------------------------------------------
relation event-inference,
  relates inferred-event,
  relates source-event;

relation entity-inference,
  relates inferred-entity,
  relates source-entity;

# ---------------------------------------------------------------------------
# Disease variants — 4-ary
# ---------------------------------------------------------------------------
# The fact ties an event to the diseased form of an entity, the normal form it
# replaces, and the functional consequence. Splitting it into binaries loses
# which normal entity the disease entity stands in for.
relation entity-functional-status,
  owns db-id @key,
  owns display-name,
  owns schema-class,
  relates affected-event @card(1..),
  relates disease-entity,
  relates normal-entity @card(0..1),
  relates functional-status @card(1..);

relation functional-status-typing,
  relates typed-status,
  relates status-type;

# ---------------------------------------------------------------------------
# Literature and curation
# ---------------------------------------------------------------------------
relation literature-citation,
  relates citing-thing,
  relates cited-publication;

relation summarisation,
  relates summarised-thing,
  relates summation;

relation publication-authorship,
  relates publication,
  relates publication-author,
  owns ordering @card(0..1);

relation person-affiliation,
  relates affiliated-person,
  relates affiliation;

# Every curation act is the same shape — an object and the edit that touched
# it — so the subtypes differ only in what the edit means. Asking "was this
# touched at all" is one query over the supertype.
relation curation @abstract,
  relates curated-object,
  relates edit;

relation creation sub curation;
relation modification sub curation;
relation authoring sub curation;
relation review sub curation;
relation internal-review sub curation;
relation revision sub curation;
# `edited` and `structureModified` are the same shape as the rest — an object
# and the edit that touched it — so they belong here rather than as standalone
# relations. Adding them makes "touched by any curation act" answer correctly.
relation editing sub curation;
relation structure-modification sub curation;

relation edit-authorship,
  relates authored-edit,
  relates edit-author,
  owns ordering @card(0..1);

relation release-record,
  relates tracked-object,
  relates tracking-release;

relation update-tracking,
  relates tracker,
  relates updated-object;

# ---------------------------------------------------------------------------
# Reference sequence cross-links
# ---------------------------------------------------------------------------
relation reference-gene-link,
  relates referring-sequence,
  relates gene-sequence;

relation reference-transcript-link,
  relates referring-sequence,
  relates transcript-sequence;

relation residue-reference-sequence,
  relates modified-residue,
  relates residue-sequence;

relation second-reference-sequence,
  relates crosslinked-residue,
  relates partner-sequence;

relation isoform-parenthood,
  relates isoform,
  relates parent-gene-product;

relation residue-modification,
  relates modified-residue,
  relates modifying-group;

relation crosslink-equivalence,
  relates crosslinked-residue,
  relates equivalent-residue;

# ---------------------------------------------------------------------------
# Interactions
# ---------------------------------------------------------------------------
relation interaction-participation,
  relates interaction,
  relates interactor;

# ---------------------------------------------------------------------------
# Cells, markers and anatomy
# ---------------------------------------------------------------------------
relation cell-marker-reference,
  relates marked-cell,
  relates cell-marker-ref;

relation marker-reference-cell,
  relates referencing-marker,
  relates referenced-cell;

relation marker-assignment,
  relates marker-ref,
  relates marker-entity;

relation protein-marker-assignment,
  relates marked-cell,
  relates protein-marker;

relation rna-marker-assignment,
  relates marked-cell,
  relates rna-marker;

relation organ-assignment,
  relates localised-cell,
  relates organ;

relation tissue-assignment,
  relates localised-thing,
  relates tissue;

relation tissue-layer-assignment,
  relates localised-cell,
  relates tissue-layer;

relation cell-type-assignment,
  relates typed-thing,
  relates assigned-cell-type;

# ---------------------------------------------------------------------------
# Compartment topology — GO cellular components relate to one another
# ---------------------------------------------------------------------------
relation compartment-membership,
  relates part-compartment,
  relates whole-compartment;

relation compartment-part,
  relates whole-compartment,
  relates part-compartment;

relation compartment-surrounding,
  relates surrounded-compartment,
  relates surrounding-compartment;

relation go-cellular-component-assignment,
  relates localised-thing,
  relates cellular-component;

# ---------------------------------------------------------------------------
# Control references — the literature evidence for a catalysis or regulation.
# The evidence points at the relation itself, which TypeDB allows: a relation
# can play a role in another relation, so no reification is reintroduced.
# ---------------------------------------------------------------------------
relation catalyst-activity-reference-link,
  relates referencing-reaction,
  relates catalyst-reference;

relation catalyst-activity-evidence,
  relates evidencing-reference,
  relates evidenced-catalysis;

relation regulation-reference-link,
  relates referencing-reaction,
  relates regulation-ref;

relation regulation-evidence,
  relates evidencing-reference,
  relates evidenced-regulation;

# 28 goBiologicalProcess edges start at a Regulation rather than an Event, so
# they cannot use go-annotation, whose annotated-event role only events play.
relation regulation-go-annotation,
  relates annotated-regulation,
  relates regulation-biological-process;

# ---------------------------------------------------------------------------
# Curation and review status
# ---------------------------------------------------------------------------
relation review-status-assignment,
  relates reviewed-thing,
  relates status;

relation previous-review-status-assignment,
  relates reviewed-thing,
  relates previous-status;

relation evidence-type-assignment,
  relates evidenced-thing,
  relates evidence;

relation figure-illustration,
  relates illustrated-thing,
  relates figure;

relation psi-mod-assignment,
  relates modified-thing,
  relates psi-mod-term;

relation structural-variant-assignment,
  relates varied-status,
  relates variant-term;

relation publication-publisher,
  relates published-work,
  relates publisher;

# ---------------------------------------------------------------------------
# Deletion bookkeeping
# ---------------------------------------------------------------------------
relation deleted-instance-record,
  relates deletion,
  relates deleted-thing;

relation replacement-instance,
  relates deletion,
  relates replacement;

relation deletion-reason,
  relates deletion,
  relates reason;

# ---------------------------------------------------------------------------
# Alternative event structure
# ---------------------------------------------------------------------------
relation encapsulated-event,
  relates encapsulating-pathway,
  relates encapsulated;

relation normal-reaction-link,
  relates disease-reaction,
  relates normal-reaction;

relation normal-pathway-link,
  relates disease-pathway,
  relates normal-pathway;

relation reverse-reaction-link,
  relates forward-reaction,
  relates reverse-reaction;

relation entity-on-other-cell,
  relates interacting-thing,
  relates other-cell-entity;

relation reaction-type-assignment,
  relates typed-reaction,
  relates assigned-reaction-type;
"""

# Role players, declared on the most general type that can play the role so
# that every subtype inherits the capability.
# Roles physical-entity plays that specialise a role it already plays.
#
# TypeQL does not infer these: declaring `plays composition:part` does not let a
# type play `candidate-membership:candidate`, even though `candidate` is
# declared `as part`. Each specialisation has to be spelled out.
#
# Kept out of PLAYS rather than as an entry in it, so every key of that dict is
# a real type label. It previously lived there under the synthetic key
# "physical-entity+specialised", which reads like a type and is not one —
# anything walking PLAYS structurally mistakes it for one.
PHYSICAL_ENTITY_SPECIALISED_PLAYS = [
    "complex-composition:component", "set-membership:member",
    "candidate-membership:candidate", "polymer-repetition:repeated-unit",
    "reaction-input:consumed-entity", "reaction-output:produced-entity",
    "required-input-component:required-component",
]

PLAYS = {
    "pathway": ["event-containment:containing-pathway", "encapsulated-event:encapsulating-pathway", "encapsulated-event:encapsulated", "normal-pathway-link:disease-pathway", "normal-pathway-link:normal-pathway"],
    "event": ["event-containment:contained-event", "event-precedence:preceding-event", "event-precedence:following-event", "negative-precedence:excluded-preceding-event", "negative-precedence:following-event", "event-inference:inferred-event", "event-inference:source-event", "species-assignment:classified-thing", "related-species-assignment:classified-thing", "compartment-assignment:localised-thing", "disease-annotation:diseased-thing", "go-annotation:annotated-event", "literature-citation:citing-thing", "summarisation:summarised-thing", "cross-reference:referring-thing"],
    "reaction-like-event": ["reaction-participation:reaction", "catalysis:catalysed-reaction", "regulation:regulated-event", "entity-functional-status:affected-event", "catalyst-activity-reference-link:referencing-reaction", "regulation-reference-link:referencing-reaction", "normal-reaction-link:disease-reaction", "normal-reaction-link:normal-reaction", "reverse-reaction-link:forward-reaction", "reverse-reaction-link:reverse-reaction", "reaction-type-assignment:typed-reaction"],
    "physical-entity": ["reaction-participation:participant", "catalysis:catalyst", "catalysis:active-unit", "regulation:regulator", "regulation:regulation-active-unit", "composition:part", "species-assignment:classified-thing", "related-species-assignment:classified-thing", "compartment-assignment:localised-thing", "disease-annotation:diseased-thing", "entity-inference:inferred-entity", "entity-inference:source-entity", "entity-functional-status:disease-entity", "entity-functional-status:normal-entity", "literature-citation:citing-thing", "summarisation:summarised-thing", "cross-reference:referring-thing", "reference-assignment:instance-entity", "entity-on-other-cell:other-cell-entity"],
    "complex": ["complex-composition:containing-complex", "included-location:localised-thing"],
    "entity-set": ["set-membership:containing-set", "included-location:localised-thing"],
    "polymer": ["polymer-repetition:containing-polymer"],
    "entity-with-accessioned-sequence": ["modified-residue-assignment:modified-entity", "marker-assignment:marker-entity", "protein-marker-assignment:protein-marker", "rna-marker-assignment:rna-marker"],
    "abstract-modified-residue": ["modified-residue-assignment:residue", "residue-reference-sequence:modified-residue", "residue-modification:modified-residue"],
    "catalyst-activity": [],
    "go-molecular-function": ["catalysis:catalytic-activity", "regulation:regulatory-activity"],
    "go-biological-process": ["regulation-go-annotation:regulation-biological-process", "go-annotation:biological-process"],
    "go-cellular-component": ["compartment-assignment:compartment", "included-location:location", "ontology-parenthood:ontology-child", "ontology-parenthood:ontology-parent", "compartment-membership:part-compartment", "compartment-membership:whole-compartment", "compartment-part:whole-compartment", "compartment-part:part-compartment", "compartment-surrounding:surrounded-compartment", "compartment-surrounding:surrounding-compartment", "go-cellular-component-assignment:cellular-component"],
    "external-ontology": ["ontology-parenthood:ontology-child", "ontology-parenthood:ontology-parent", "disease-annotation:disease"],
    "taxon": ["taxonomy-parenthood:sub-taxon", "taxonomy-parenthood:super-taxon", "species-assignment:species"],
    "species": ["related-species-assignment:related-species"],
    "reference-entity": ["reference-assignment:reference", "cross-reference:referring-thing", "species-assignment:classified-thing", "database-of:external-thing", "interaction-participation:interactor", "residue-modification:modifying-group"],
    "reference-database": ["database-of:reference-database"],
    "database-identifier": ["cross-reference:external-identifier", "database-of:external-thing"],
    "publication": ["literature-citation:cited-publication", "publication-authorship:publication"],
    "person": ["publication-authorship:publication-author", "edit-authorship:edit-author", "person-affiliation:affiliated-person"],
    "affiliation": ["person-affiliation:affiliation", "publication-publisher:publisher"],
    "instance-edit": ["curation:edit", "edit-authorship:authored-edit"],
    "database-object": ["curation:curated-object", "release-record:tracked-object", "update-tracking:updated-object", "review-status-assignment:reviewed-thing", "previous-review-status-assignment:reviewed-thing", "evidence-type-assignment:evidenced-thing", "figure-illustration:illustrated-thing", "psi-mod-assignment:modified-thing", "cell-type-assignment:typed-thing", "tissue-assignment:localised-thing", "go-cellular-component-assignment:localised-thing", "entity-on-other-cell:interacting-thing", "replacement-instance:replacement"],
    "update-tracker": ["release-record:tracked-object", "update-tracking:tracker"],
    "release": ["release-record:tracking-release"],
    "deleted": ["deleted-instance-record:deletion", "replacement-instance:deletion", "deletion-reason:deletion"],
    "deleted-instance": ["deleted-instance-record:deleted-thing"],
    "deleted-controlled-vocabulary": ["deletion-reason:reason"],
    "review-status": ["review-status-assignment:status", "previous-review-status-assignment:previous-status"],
    "evidence-type": ["evidence-type-assignment:evidence"],
    "figure": ["figure-illustration:figure"],
    "psi-mod": ["psi-mod-assignment:psi-mod-term"],
    "cell-type": ["cell-type-assignment:assigned-cell-type"],
    "anatomy": ["organ-assignment:organ", "tissue-assignment:tissue", "tissue-layer-assignment:tissue-layer"],
    "sequence-ontology": ["structural-variant-assignment:variant-term"],
    "functional-status": ["structural-variant-assignment:varied-status", "entity-functional-status:functional-status", "functional-status-typing:typed-status"],
    "reaction-type": ["reaction-type-assignment:assigned-reaction-type"],
    "book": ["publication-publisher:published-work"],
    "interaction": ["interaction-participation:interaction"],
    "catalyst-activity-reference": ["catalyst-activity-reference-link:catalyst-reference", "catalyst-activity-evidence:evidencing-reference"],
    "regulation-reference": ["regulation-reference-link:regulation-ref", "regulation-evidence:evidencing-reference"],
    "marker-reference": ["cell-marker-reference:cell-marker-ref", "marker-reference-cell:referencing-marker", "marker-assignment:marker-ref"],
    "summation": ["summarisation:summation", "literature-citation:citing-thing"],
    "cell": ["cell-marker-reference:marked-cell", "marker-reference-cell:referenced-cell", "protein-marker-assignment:marked-cell", "rna-marker-assignment:marked-cell", "organ-assignment:localised-cell", "tissue-layer-assignment:localised-cell"],
    "reference-sequence": ["reference-gene-link:referring-sequence", "reference-transcript-link:referring-sequence", "residue-reference-sequence:residue-sequence", "second-reference-sequence:partner-sequence"],
    "reference-dnasequence": ["reference-gene-link:gene-sequence"],
    "reference-rnasequence": ["reference-transcript-link:transcript-sequence"],
    "reference-isoform": ["isoform-parenthood:isoform"],
    "reference-gene-product": ["isoform-parenthood:parent-gene-product"],
    "inter-chain-crosslinked-residue": ["second-reference-sequence:crosslinked-residue", "crosslink-equivalence:crosslinked-residue", "crosslink-equivalence:equivalent-residue"],
    "functional-status-type": ["functional-status-typing:status-type"],
    "negative-preceding-event-reason": ["negative-precedence:exclusion-reason"],
}

# Reactome reifies these as classes because neither the relational model nor a
# property graph can state an n-ary fact directly. Here they ARE the relation,
# so they must not also become entities.
AS_RELATIONS = {
    "Regulation", "PositiveRegulation", "NegativeRegulation", "Requirement",
    "PositiveGeneExpressionRegulation", "NegativeGeneExpressionRegulation",
    "CatalystActivity", "EntityFunctionalStatus", "NegativePrecedingEvent",
}

# Where two labels are co-extensive the data cannot tell them apart, so only
# one is kept as a type; keeping both would let a node belong to two types.
COEXTENSIVE_LOSERS = {"UndirectedInteraction", "DrugActionType", "TranscriptionalModification"}


def _merge_specialised(plays: dict[str, list[str]]) -> dict[str, list[str]]:
    """Fold the specialised roles into physical-entity's own `plays`."""
    plays["physical-entity"] = sorted(
        set(plays.get("physical-entity", []) + PHYSICAL_ENTITY_SPECIALISED_PLAYS))
    return plays


def hoist_roles(plays: dict[str, list[str]], parent: dict[str, str | None]) -> dict[str, list[str]]:
    """Declare each role once, on the nearest common ancestor of its players.

    TypeDB type-checks role compatibility when a query is compiled, not when a
    row is inserted: a loader pass that matches its player at a type which does
    not itself play the role is rejected wholesale, however correct the data.
    Declaring `species-assignment:classified-thing` separately on event,
    physical-entity and reference-entity therefore leaves no single type a pass
    can match at, so the role moves up to the type that covers all three.
    """
    label_of = kebab

    chain: dict[str, list[str]] = {}
    for name in parent:
        lbl, path, cur = label_of(name), [], name
        while cur:
            path.append(label_of(cur))
            cur = parent.get(cur)
        chain[lbl] = path                     # self first, root last

    owners: dict[str, list[str]] = {}
    for entity, roles in plays.items():
        for role in roles:
            owners.setdefault(role, []).append(entity)

    hoisted: dict[str, list[str]] = {}
    for role, ents in owners.items():
        known = [e for e in ents if e in chain]
        if not known:
            continue
        common = [t for t in chain[known[0]] if all(t in chain[e] for e in known)]
        target = common[0] if common else "database-object"
        hoisted.setdefault(target, []).append(role)
    return {k: sorted(v) for k, v in hoisted.items()}


def main() -> None:
    dst = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).with_name("schema.tql")
    parent = read_hierarchy()

    label = kebab

    # Reactome's own root is DatabaseObject; everything hangs off it.
    order: list[str] = []
    seen: set[str] = set()

    def emit(name: str) -> None:
        if name in seen:
            return
        p = parent.get(name)
        while p in AS_RELATIONS:          # skip over reified classes
            p = parent.get(p)
        if p:
            emit(p)
        seen.add(name)
        order.append(name)

    for name in sorted(parent):
        if name in AS_RELATIONS or name in MIXINS or name in COEXTENSIVE_LOSERS:
            continue
        emit(name)

    lines = [
        "# Reactome schema for TypeDB 3.12.",
        "#",
        "# The entity hierarchy mirrors Reactome's own class hierarchy, so a query",
        "# against a supertype (physical-entity, regulation, composition) also matches",
        "# every subtype. Relations are n-ary where the fact is n-ary: catalysis ties a",
        "# catalyst, a molecular function and a reaction in one relation, and",
        "# entity-functional-status ties an event, a diseased entity, the normal entity",
        "# it replaces and the functional consequence.",
        "",
        "define",
        "",
        "# --- attributes ---",
    ]
    lines += [l for l in ATTRIBUTES.strip().splitlines()]
    lines += ["", "# --- entity hierarchy (mirrors Reactome's classes) ---", ""]

    plays_map = hoist_roles(_merge_specialised(dict(PLAYS)), parent)
    for name in order:
        lbl = label(name)
        p = parent.get(name)
        while p in AS_RELATIONS:
            p = parent.get(p)
        head = f"entity {lbl}" if not p else f"entity {lbl} sub {label(p)}"
        owns = OWNERSHIP.get(lbl, [])
        plays = plays_map.get(lbl, [])
        parts = [head]
        if name == "DatabaseObject":
            parts[0] += " @abstract"
        for o in owns:
            parts.append(f"  owns {o}")
        for pl in plays:
            parts.append(f"  plays {pl}")
        lines.append(",\n".join(parts) + ";")

    lines += ["", "# --- relations ---"]
    lines += RELATIONS.strip().splitlines()

    text = "\n".join(lines) + "\n"
    dst.write_text(text, encoding="utf8")
    print(f"wrote {dst} — {len(order)} entity types, {text.count('relation ')} relation defs, "
          f"{len(text)} chars, ~{len(text)//4} est. tokens")


if __name__ == "__main__":
    main()
