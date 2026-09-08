CREATE TABLE `AbstractModifiedResidue` (
  `DB_ID` int unsigned NOT NULL,
  `referenceSequence` int unsigned DEFAULT NULL,
  `referenceSequence_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceSequence` (`referenceSequence`)
);

CREATE TABLE `Affiliation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `address` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `address` (`address`(10))
);

CREATE TABLE `Affiliation_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `Anatomy` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `BlackBoxEvent` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `templateEvent` int unsigned DEFAULT NULL,
  `templateEvent_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `templateEvent` (`templateEvent`)
);

CREATE TABLE `Book` (
  `DB_ID` int unsigned NOT NULL,
  `ISBN` mediumtext,
  `chapterTitle` mediumtext,
  `pages` mediumtext,
  `publisher` int unsigned DEFAULT NULL,
  `publisher_class` varchar(64) DEFAULT NULL,
  `year` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `ISBN` (`ISBN`(10)),
  KEY `chapterTitle` (`chapterTitle`(10)),
  KEY `pages` (`pages`(10)),
  KEY `publisher` (`publisher`),
  KEY `year` (`year`)
);

CREATE TABLE `Book_2_chapterAuthors` (
  `DB_ID` int unsigned DEFAULT NULL,
  `chapterAuthors_rank` int unsigned DEFAULT NULL,
  `chapterAuthors` int unsigned DEFAULT NULL,
  `chapterAuthors_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `chapterAuthors` (`chapterAuthors`)
);

CREATE TABLE `CandidateSet` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `CandidateSet_2_hasCandidate` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasCandidate_rank` int unsigned DEFAULT NULL,
  `hasCandidate` int unsigned DEFAULT NULL,
  `hasCandidate_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasCandidate` (`hasCandidate`)
);

CREATE TABLE `CatalystActivity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `activity` int unsigned DEFAULT NULL,
  `activity_class` varchar(64) DEFAULT NULL,
  `physicalEntity` int unsigned DEFAULT NULL,
  `physicalEntity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `activity` (`activity`),
  KEY `physicalEntity` (`physicalEntity`)
);

CREATE TABLE `CatalystActivityReference` (
  `DB_ID` int unsigned NOT NULL,
  `catalystActivity` int unsigned DEFAULT NULL,
  `catalystActivity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `catalystActivity` (`catalystActivity`)
);

CREATE TABLE `CatalystActivity_2_activeUnit` (
  `DB_ID` int unsigned DEFAULT NULL,
  `activeUnit_rank` int unsigned DEFAULT NULL,
  `activeUnit` int unsigned DEFAULT NULL,
  `activeUnit_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `activeUnit` (`activeUnit`)
);

CREATE TABLE `Cell` (
  `DB_ID` int unsigned NOT NULL,
  `organ` int unsigned DEFAULT NULL,
  `organ_class` varchar(64) DEFAULT NULL,
  `tissue` int unsigned DEFAULT NULL,
  `tissue_class` varchar(64) DEFAULT NULL,
  `tissueLayer` int unsigned DEFAULT NULL,
  `tissueLayer_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `organ` (`organ`),
  KEY `tissue` (`tissue`),
  KEY `tissueLayer` (`tissueLayer`)
);

CREATE TABLE `CellDevelopmentStep` (
  `DB_ID` int unsigned NOT NULL,
  `tissue` int unsigned DEFAULT NULL,
  `tissue_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `tissue` (`tissue`)
);

CREATE TABLE `CellLineagePath` (
  `DB_ID` int unsigned NOT NULL,
  `tissue` int unsigned DEFAULT NULL,
  `tissue_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `tissue` (`tissue`)
);

CREATE TABLE `CellType` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Cell_2_RNAMarker` (
  `DB_ID` int unsigned DEFAULT NULL,
  `RNAMarker_rank` int unsigned DEFAULT NULL,
  `RNAMarker` int unsigned DEFAULT NULL,
  `RNAMarker_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `RNAMarker` (`RNAMarker`)
);

CREATE TABLE `Cell_2_markerReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `markerReference_rank` int unsigned DEFAULT NULL,
  `markerReference` int unsigned DEFAULT NULL,
  `markerReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `markerReference` (`markerReference`)
);

CREATE TABLE `Cell_2_proteinMarker` (
  `DB_ID` int unsigned DEFAULT NULL,
  `proteinMarker_rank` int unsigned DEFAULT NULL,
  `proteinMarker` int unsigned DEFAULT NULL,
  `proteinMarker_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `proteinMarker` (`proteinMarker`)
);

CREATE TABLE `Cell_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `ChemicalDrug` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Compartment` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Complex` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `isChimeric` enum('TRUE','FALSE') DEFAULT NULL,
  `stoichiometryKnown` enum('TRUE','FALSE') DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `isChimeric` (`isChimeric`),
  KEY `stoichiometryKnown` (`stoichiometryKnown`)
);

CREATE TABLE `Complex_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `Complex_2_componentCellType` (
  `DB_ID` int unsigned DEFAULT NULL,
  `componentCellType_rank` int unsigned DEFAULT NULL,
  `componentCellType` int unsigned DEFAULT NULL,
  `componentCellType_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `componentCellType` (`componentCellType`)
);

CREATE TABLE `Complex_2_entityOnOtherCell` (
  `DB_ID` int unsigned DEFAULT NULL,
  `entityOnOtherCell_rank` int unsigned DEFAULT NULL,
  `entityOnOtherCell` int unsigned DEFAULT NULL,
  `entityOnOtherCell_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `entityOnOtherCell` (`entityOnOtherCell`)
);

CREATE TABLE `Complex_2_hasComponent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasComponent_rank` int unsigned DEFAULT NULL,
  `hasComponent` int unsigned DEFAULT NULL,
  `hasComponent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasComponent` (`hasComponent`)
);

CREATE TABLE `Complex_2_includedLocation` (
  `DB_ID` int unsigned DEFAULT NULL,
  `includedLocation_rank` int unsigned DEFAULT NULL,
  `includedLocation` int unsigned DEFAULT NULL,
  `includedLocation_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `includedLocation` (`includedLocation`)
);

CREATE TABLE `Complex_2_relatedSpecies` (
  `DB_ID` int unsigned DEFAULT NULL,
  `relatedSpecies_rank` int unsigned DEFAULT NULL,
  `relatedSpecies` int unsigned DEFAULT NULL,
  `relatedSpecies_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `relatedSpecies` (`relatedSpecies`)
);

CREATE TABLE `Complex_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `ControlReference` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ControlReference_2_literatureReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `literatureReference_rank` int unsigned DEFAULT NULL,
  `literatureReference` int unsigned DEFAULT NULL,
  `literatureReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `literatureReference` (`literatureReference`)
);

CREATE TABLE `ControlledVocabulary` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `definition` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `definition` (`definition`(10))
);

CREATE TABLE `ControlledVocabulary_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `CrosslinkedResidue` (
  `DB_ID` int unsigned NOT NULL,
  `modification` int unsigned DEFAULT NULL,
  `modification_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `modification` (`modification`)
);

CREATE TABLE `CrosslinkedResidue_2_secondCoordinate` (
  `DB_ID` int unsigned DEFAULT NULL,
  `secondCoordinate_rank` int unsigned DEFAULT NULL,
  `secondCoordinate` int DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `secondCoordinate` (`secondCoordinate`)
);

CREATE TABLE `DataModel` (
  `thing` varchar(255) NOT NULL,
  `thing_class` enum('SchemaClass','SchemaClassAttribute','Schema') DEFAULT NULL,
  `property_name` varchar(255) NOT NULL,
  `property_value` text,
  `property_value_type` enum('INTEGER','SYMBOL','STRING','INSTANCE','SchemaClass','SchemaClassAttribute') DEFAULT NULL,
  `property_value_rank` int unsigned DEFAULT '0'
);

CREATE TABLE `DatabaseIdentifier` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `identifier` varchar(50) DEFAULT NULL,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `identifier` (`identifier`),
  KEY `referenceDatabase` (`referenceDatabase`)
);

CREATE TABLE `DatabaseIdentifier_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `DatabaseObject` (
  `DB_ID` int NOT NULL AUTO_INCREMENT,
  `_class` varchar(64) DEFAULT NULL,
  `_displayName` mediumtext,
  `_timestamp` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  `created` int unsigned DEFAULT NULL,
  `created_class` varchar(64) DEFAULT NULL,
  `stableIdentifier` int unsigned DEFAULT NULL,
  `stableIdentifier_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `_class` (`_class`),
  KEY `_timestamp` (`_timestamp`),
  KEY `created` (`created`),
  KEY `_displayName` (`_displayName`(10)),
  KEY `stableIdentifier` (`stableIdentifier`)
);

CREATE TABLE `DatabaseObject_2_modified` (
  `DB_ID` int unsigned DEFAULT NULL,
  `modified_rank` int unsigned DEFAULT NULL,
  `modified` int unsigned DEFAULT NULL,
  `modified_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `modified` (`modified`)
);

CREATE TABLE `DefinedSet` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `DeletedControlledVocabulary` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Depolymerisation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Disease` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Drug` (
  `DB_ID` int unsigned NOT NULL,
  `referenceEntity` int unsigned DEFAULT NULL,
  `referenceEntity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceEntity` (`referenceEntity`)
);

CREATE TABLE `DrugActionType` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `DrugActionType_2_instanceOf` (
  `DB_ID` int unsigned DEFAULT NULL,
  `instanceOf_rank` int unsigned DEFAULT NULL,
  `instanceOf` int unsigned DEFAULT NULL,
  `instanceOf_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `instanceOf` (`instanceOf`)
);

CREATE TABLE `Drug_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `EntityFunctionalStatus` (
  `DB_ID` int unsigned NOT NULL,
  `diseaseEntity` int unsigned DEFAULT NULL,
  `diseaseEntity_class` varchar(64) DEFAULT NULL,
  `normalEntity` int unsigned DEFAULT NULL,
  `normalEntity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `diseaseEntity` (`diseaseEntity`),
  KEY `normalEntity` (`normalEntity`)
);

CREATE TABLE `EntityFunctionalStatus_2_functionalStatus` (
  `DB_ID` int unsigned DEFAULT NULL,
  `functionalStatus_rank` int unsigned DEFAULT NULL,
  `functionalStatus` int unsigned DEFAULT NULL,
  `functionalStatus_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `functionalStatus` (`functionalStatus`)
);

CREATE TABLE `EntitySet` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `isOrdered` enum('TRUE','FALSE') DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `isOrdered` (`isOrdered`)
);

CREATE TABLE `EntitySet_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `EntitySet_2_hasMember` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasMember_rank` int unsigned DEFAULT NULL,
  `hasMember` int unsigned DEFAULT NULL,
  `hasMember_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasMember` (`hasMember`)
);

CREATE TABLE `EntitySet_2_relatedSpecies` (
  `DB_ID` int unsigned DEFAULT NULL,
  `relatedSpecies_rank` int unsigned DEFAULT NULL,
  `relatedSpecies` int unsigned DEFAULT NULL,
  `relatedSpecies_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `relatedSpecies` (`relatedSpecies`)
);

CREATE TABLE `EntitySet_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `EntityWithAccessionedSequence` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `endCoordinate` int DEFAULT NULL,
  `referenceEntity` int unsigned DEFAULT NULL,
  `referenceEntity_class` varchar(64) DEFAULT NULL,
  `startCoordinate` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `endCoordinate` (`endCoordinate`),
  KEY `referenceEntity` (`referenceEntity`),
  KEY `startCoordinate` (`startCoordinate`)
);

CREATE TABLE `EntityWithAccessionedSequence_2_hasModifiedResidue` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasModifiedResidue_rank` int unsigned DEFAULT NULL,
  `hasModifiedResidue` int unsigned DEFAULT NULL,
  `hasModifiedResidue_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasModifiedResidue` (`hasModifiedResidue`)
);

CREATE TABLE `Event` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `definition` mediumtext,
  `evidenceType` int unsigned DEFAULT NULL,
  `evidenceType_class` varchar(64) DEFAULT NULL,
  `goBiologicalProcess` int unsigned DEFAULT NULL,
  `goBiologicalProcess_class` varchar(64) DEFAULT NULL,
  `releaseDate` date DEFAULT NULL,
  `_doRelease` enum('TRUE','FALSE') DEFAULT NULL,
  `releaseStatus` mediumtext,
  `previousReviewStatus` int unsigned DEFAULT NULL,
  `previousReviewStatus_class` varchar(64) DEFAULT NULL,
  `reviewStatus` int unsigned DEFAULT NULL,
  `reviewStatus_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `evidenceType` (`evidenceType`),
  KEY `goBiologicalProcess` (`goBiologicalProcess`),
  KEY `definition` (`definition`(10)),
  KEY `releaseDate` (`releaseDate`),
  KEY `_doRelease` (`_doRelease`),
  KEY `releaseStatus` (`releaseStatus`(10)),
  KEY `previousReviewStatus` (`previousReviewStatus`),
  KEY `reviewStatus` (`reviewStatus`)
);

CREATE TABLE `Event_2_authored` (
  `DB_ID` int unsigned DEFAULT NULL,
  `authored_rank` int unsigned DEFAULT NULL,
  `authored` int unsigned DEFAULT NULL,
  `authored_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `authored` (`authored`)
);

CREATE TABLE `Event_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `Event_2_disease` (
  `DB_ID` int unsigned DEFAULT NULL,
  `disease_rank` int unsigned DEFAULT NULL,
  `disease` int unsigned DEFAULT NULL,
  `disease_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `disease` (`disease`)
);

CREATE TABLE `Event_2_edited` (
  `DB_ID` int unsigned DEFAULT NULL,
  `edited_rank` int unsigned DEFAULT NULL,
  `edited` int unsigned DEFAULT NULL,
  `edited_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `edited` (`edited`)
);

CREATE TABLE `Event_2_figure` (
  `DB_ID` int unsigned DEFAULT NULL,
  `figure_rank` int unsigned DEFAULT NULL,
  `figure` int unsigned DEFAULT NULL,
  `figure_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `figure` (`figure`)
);

CREATE TABLE `Event_2_inferredFrom` (
  `DB_ID` int unsigned DEFAULT NULL,
  `inferredFrom_rank` int unsigned DEFAULT NULL,
  `inferredFrom` int unsigned DEFAULT NULL,
  `inferredFrom_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `inferredFrom` (`inferredFrom`)
);

CREATE TABLE `Event_2_internalReviewed` (
  `DB_ID` int unsigned DEFAULT NULL,
  `internalReviewed_rank` int unsigned DEFAULT NULL,
  `internalReviewed` int unsigned DEFAULT NULL,
  `internalReviewed_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `internalReviewed` (`internalReviewed`)
);

CREATE TABLE `Event_2_literatureReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `literatureReference_rank` int unsigned DEFAULT NULL,
  `literatureReference` int unsigned DEFAULT NULL,
  `literatureReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `literatureReference` (`literatureReference`)
);

CREATE TABLE `Event_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `Event_2_negativePrecedingEvent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `negativePrecedingEvent_rank` int unsigned DEFAULT NULL,
  `negativePrecedingEvent` int unsigned DEFAULT NULL,
  `negativePrecedingEvent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `negativePrecedingEvent` (`negativePrecedingEvent`)
);

CREATE TABLE `Event_2_orthologousEvent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `orthologousEvent_rank` int unsigned DEFAULT NULL,
  `orthologousEvent` int unsigned DEFAULT NULL,
  `orthologousEvent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `orthologousEvent` (`orthologousEvent`)
);

CREATE TABLE `Event_2_precedingEvent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `precedingEvent_rank` int unsigned DEFAULT NULL,
  `precedingEvent` int unsigned DEFAULT NULL,
  `precedingEvent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `precedingEvent` (`precedingEvent`)
);

CREATE TABLE `Event_2_relatedSpecies` (
  `DB_ID` int unsigned DEFAULT NULL,
  `relatedSpecies_rank` int unsigned DEFAULT NULL,
  `relatedSpecies` int unsigned DEFAULT NULL,
  `relatedSpecies_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `relatedSpecies` (`relatedSpecies`)
);

CREATE TABLE `Event_2_reviewed` (
  `DB_ID` int unsigned DEFAULT NULL,
  `reviewed_rank` int unsigned DEFAULT NULL,
  `reviewed` int unsigned DEFAULT NULL,
  `reviewed_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `reviewed` (`reviewed`)
);

CREATE TABLE `Event_2_revised` (
  `DB_ID` int unsigned DEFAULT NULL,
  `revised_rank` int unsigned DEFAULT NULL,
  `revised` int unsigned DEFAULT NULL,
  `revised_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `revised` (`revised`)
);

CREATE TABLE `Event_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `Event_2_structureModified` (
  `DB_ID` int unsigned DEFAULT NULL,
  `structureModified_rank` int unsigned DEFAULT NULL,
  `structureModified` int unsigned DEFAULT NULL,
  `structureModified_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `structureModified` (`structureModified`)
);

CREATE TABLE `Event_2_summation` (
  `DB_ID` int unsigned DEFAULT NULL,
  `summation_rank` int unsigned DEFAULT NULL,
  `summation` int unsigned DEFAULT NULL,
  `summation_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `summation` (`summation`)
);

CREATE TABLE `EvidenceType` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ExternalOntology` (
  `DB_ID` int unsigned NOT NULL,
  `definition` mediumtext,
  `identifier` mediumtext,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `definition` (`definition`(10)),
  KEY `identifier` (`identifier`(10)),
  KEY `referenceDatabase` (`referenceDatabase`)
);

CREATE TABLE `ExternalOntology_2_instanceOf` (
  `DB_ID` int unsigned DEFAULT NULL,
  `instanceOf_rank` int unsigned DEFAULT NULL,
  `instanceOf` int unsigned DEFAULT NULL,
  `instanceOf_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `instanceOf` (`instanceOf`)
);

CREATE TABLE `ExternalOntology_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `ExternalOntology_2_synonym` (
  `DB_ID` int unsigned DEFAULT NULL,
  `synonym_rank` int unsigned DEFAULT NULL,
  `synonym` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `synonym` (`synonym`(10))
);

CREATE TABLE `FailedReaction` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Figure` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `url` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `url` (`url`(10))
);

CREATE TABLE `FragmentDeletionModification` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `FragmentInsertionModification` (
  `DB_ID` int unsigned NOT NULL,
  `coordinate` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `coordinate` (`coordinate`)
);

CREATE TABLE `FragmentModification` (
  `DB_ID` int unsigned NOT NULL,
  `endPositionInReferenceSequence` int DEFAULT NULL,
  `startPositionInReferenceSequence` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `endPositionInReferenceSequence` (`endPositionInReferenceSequence`),
  KEY `startPositionInReferenceSequence` (`startPositionInReferenceSequence`)
);

CREATE TABLE `FragmentReplacedModification` (
  `DB_ID` int unsigned NOT NULL,
  `alteredAminoAcidFragment` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `alteredAminoAcidFragment` (`alteredAminoAcidFragment`(10))
);

CREATE TABLE `FrontPage` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `FrontPage_2_frontPageItem` (
  `DB_ID` int unsigned DEFAULT NULL,
  `frontPageItem_rank` int unsigned DEFAULT NULL,
  `frontPageItem` int unsigned DEFAULT NULL,
  `frontPageItem_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `frontPageItem` (`frontPageItem`)
);

CREATE TABLE `FunctionalStatus` (
  `DB_ID` int unsigned NOT NULL,
  `functionalStatusType` int unsigned DEFAULT NULL,
  `structuralVariant` int unsigned DEFAULT NULL,
  `structuralVariant_class` varchar(64) DEFAULT NULL,
  `functionalStatusType_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `structuralVariant` (`structuralVariant`),
  KEY `functionalStatusType` (`functionalStatusType`)
);

CREATE TABLE `FunctionalStatusType` (
  `DB_ID` int unsigned NOT NULL,
  `definition` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `definition` (`definition`(10))
);

CREATE TABLE `FunctionalStatusType_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `GO_BiologicalProcess` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `accession` mediumtext,
  `definition` mediumtext,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceDatabase` (`referenceDatabase`),
  KEY `accession` (`accession`(10)),
  KEY `definition` (`definition`(10))
);

CREATE TABLE `GO_BiologicalProcess_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `GO_CellularComponent` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `accession` mediumtext,
  `definition` mediumtext,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceDatabase` (`referenceDatabase`),
  KEY `accession` (`accession`(10)),
  KEY `definition` (`definition`(10))
);

CREATE TABLE `GO_CellularComponent_2_componentOf` (
  `DB_ID` int unsigned DEFAULT NULL,
  `componentOf_rank` int unsigned DEFAULT NULL,
  `componentOf` int unsigned DEFAULT NULL,
  `componentOf_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `componentOf` (`componentOf`)
);

CREATE TABLE `GO_CellularComponent_2_hasPart` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasPart_rank` int unsigned DEFAULT NULL,
  `hasPart` int unsigned DEFAULT NULL,
  `hasPart_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasPart` (`hasPart`)
);

CREATE TABLE `GO_CellularComponent_2_instanceOf` (
  `DB_ID` int unsigned DEFAULT NULL,
  `instanceOf_rank` int unsigned DEFAULT NULL,
  `instanceOf` int unsigned DEFAULT NULL,
  `instanceOf_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `instanceOf` (`instanceOf`)
);

CREATE TABLE `GO_CellularComponent_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `GO_CellularComponent_2_surroundedBy` (
  `DB_ID` int unsigned DEFAULT NULL,
  `surroundedBy_rank` int unsigned DEFAULT NULL,
  `surroundedBy` int unsigned DEFAULT NULL,
  `surroundedBy_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `surroundedBy` (`surroundedBy`)
);

CREATE TABLE `GO_MolecularFunction` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `accession` mediumtext,
  `definition` mediumtext,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceDatabase` (`referenceDatabase`),
  KEY `accession` (`accession`(10)),
  KEY `definition` (`definition`(10))
);

CREATE TABLE `GO_MolecularFunction_2_ecNumber` (
  `DB_ID` int unsigned DEFAULT NULL,
  `ecNumber_rank` int unsigned DEFAULT NULL,
  `ecNumber` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `ecNumber` (`ecNumber`(10))
);

CREATE TABLE `GO_MolecularFunction_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `GeneticallyModifiedResidue` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `GenomeEncodedEntity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `GenomeEncodedEntity_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `GroupModifiedResidue` (
  `DB_ID` int unsigned NOT NULL,
  `modification` int unsigned DEFAULT NULL,
  `modification_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `modification` (`modification`)
);

CREATE TABLE `InstanceEdit` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `_applyToAllEditedInstances` mediumtext,
  `dateTime` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  `note` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `dateTime` (`dateTime`),
  KEY `_applyToAllEditedInstances` (`_applyToAllEditedInstances`(10)),
  KEY `note` (`note`(10))
);

CREATE TABLE `InstanceEdit_2_author` (
  `DB_ID` int unsigned DEFAULT NULL,
  `author_rank` int unsigned DEFAULT NULL,
  `author` int unsigned DEFAULT NULL,
  `author_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `author` (`author`)
);

CREATE TABLE `InterChainCrosslinkedResidue` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `InterChainCrosslinkedResidue_2_equivalentTo` (
  `DB_ID` int unsigned DEFAULT NULL,
  `equivalentTo_rank` int unsigned DEFAULT NULL,
  `equivalentTo` int unsigned DEFAULT NULL,
  `equivalentTo_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `equivalentTo` (`equivalentTo`)
);

CREATE TABLE `InterChainCrosslinkedResidue_2_secondReferenceSequence` (
  `DB_ID` int unsigned DEFAULT NULL,
  `secondReferenceSequence_rank` int unsigned DEFAULT NULL,
  `secondReferenceSequence` int unsigned DEFAULT NULL,
  `secondReferenceSequence_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `secondReferenceSequence` (`secondReferenceSequence`)
);

CREATE TABLE `InteractionEvent` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `InteractionEvent_2_partners` (
  `DB_ID` int unsigned DEFAULT NULL,
  `partners_rank` int unsigned DEFAULT NULL,
  `partners` int unsigned DEFAULT NULL,
  `partners_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `partners` (`partners`)
);

CREATE TABLE `IntraChainCrosslinkedResidue` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `LiteratureReference` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `journal` varchar(255) DEFAULT NULL,
  `pages` mediumtext,
  `pubMedIdentifier` int DEFAULT NULL,
  `volume` int DEFAULT NULL,
  `year` int DEFAULT NULL,
  `retractionStatus` int unsigned DEFAULT NULL,
  `retractionStatus_class` varchar(64) DEFAULT NULL,
  `comment` text,
  PRIMARY KEY (`DB_ID`),
  KEY `journal` (`journal`),
  KEY `pubMedIdentifier` (`pubMedIdentifier`),
  KEY `volume` (`volume`),
  KEY `year` (`year`),
  KEY `pages` (`pages`(10)),
  KEY `retractionStatus` (`retractionStatus`),
  KEY `comment` (`comment`(10))
);

CREATE TABLE `MarkerReference` (
  `DB_ID` int unsigned NOT NULL,
  `marker` int unsigned DEFAULT NULL,
  `marker_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `marker` (`marker`)
);

CREATE TABLE `MarkerReference_2_cell` (
  `DB_ID` int unsigned DEFAULT NULL,
  `cell_rank` int unsigned DEFAULT NULL,
  `cell` int unsigned DEFAULT NULL,
  `cell_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `cell` (`cell`)
);

CREATE TABLE `ModifiedNucleotide` (
  `DB_ID` int unsigned NOT NULL,
  `coordinate` int DEFAULT NULL,
  `modification` int unsigned DEFAULT NULL,
  `modification_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `coordinate` (`coordinate`),
  KEY `modification` (`modification`)
);

CREATE TABLE `ModifiedResidue` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `NegativeGeneExpressionRegulation` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `NegativePrecedingEvent` (
  `DB_ID` int unsigned NOT NULL,
  `comment` text,
  `reason` int unsigned DEFAULT NULL,
  `reason_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `comment` (`comment`(10)),
  KEY `reason` (`reason`)
);

CREATE TABLE `NegativePrecedingEventReason` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `NegativePrecedingEvent_2_precedingEvent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `precedingEvent_rank` int unsigned DEFAULT NULL,
  `precedingEvent` int unsigned DEFAULT NULL,
  `precedingEvent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `precedingEvent` (`precedingEvent`)
);

CREATE TABLE `NegativeRegulation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `NonsenseMutation` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Ontology` (
  `ontology` longblob
);

CREATE TABLE `OtherEntity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `OtherEntity_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `Pathway` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `doi` varchar(64) DEFAULT NULL,
  `isCanonical` enum('TRUE','FALSE') DEFAULT NULL,
  `normalPathway` int unsigned DEFAULT NULL,
  `normalPathway_class` varchar(64) DEFAULT NULL,
  `hasEHLD` text,
  `lastUpdatedDate` text,
  `cell` int unsigned DEFAULT NULL,
  `cell_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `doi` (`doi`),
  KEY `isCanonical` (`isCanonical`),
  KEY `normalPathway` (`normalPathway`),
  KEY `hasEHLD` (`hasEHLD`(10)),
  KEY `lastUpdatedDate` (`lastUpdatedDate`(10)),
  KEY `cell` (`cell`)
);

CREATE TABLE `PathwayDiagram` (
  `DB_ID` int unsigned NOT NULL,
  `height` int DEFAULT NULL,
  `storedATXML` longblob,
  `width` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `height` (`height`),
  KEY `width` (`width`)
);

CREATE TABLE `PathwayDiagram_2_representedPathway` (
  `DB_ID` int unsigned DEFAULT NULL,
  `representedPathway_rank` int unsigned DEFAULT NULL,
  `representedPathway` int unsigned DEFAULT NULL,
  `representedPathway_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `representedPathway` (`representedPathway`)
);

CREATE TABLE `Pathway_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `Pathway_2_hasEvent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasEvent_rank` int unsigned DEFAULT NULL,
  `hasEvent` int unsigned DEFAULT NULL,
  `hasEvent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasEvent` (`hasEvent`)
);

CREATE TABLE `Person` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `firstname` mediumtext,
  `initial` varchar(10) DEFAULT NULL,
  `surname` varchar(255) DEFAULT NULL,
  `project` mediumtext,
  `url` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `initial` (`initial`),
  KEY `surname` (`surname`),
  KEY `firstname` (`firstname`(10)),
  KEY `project` (`project`(10)),
  KEY `url` (`url`(10))
);

CREATE TABLE `Person_2_affiliation` (
  `DB_ID` int unsigned DEFAULT NULL,
  `affiliation_rank` int unsigned DEFAULT NULL,
  `affiliation` int unsigned DEFAULT NULL,
  `affiliation_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `affiliation` (`affiliation`)
);

CREATE TABLE `Person_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `Person_2_figure` (
  `DB_ID` int unsigned DEFAULT NULL,
  `figure_rank` int unsigned DEFAULT NULL,
  `figure` int unsigned DEFAULT NULL,
  `figure_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `figure` (`figure`)
);

CREATE TABLE `PhysicalEntity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `definition` mediumtext,
  `goCellularComponent` int unsigned DEFAULT NULL,
  `goCellularComponent_class` varchar(64) DEFAULT NULL,
  `authored` int unsigned DEFAULT NULL,
  `authored_class` varchar(64) DEFAULT NULL,
  `systematicName` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `goCellularComponent` (`goCellularComponent`),
  KEY `definition` (`definition`(10)),
  KEY `authored` (`authored`),
  KEY `systematicName` (`systematicName`(10))
);

CREATE TABLE `PhysicalEntityCellType` (
  `DB_ID` int unsigned NOT NULL,
  `cell` int unsigned DEFAULT NULL,
  `cell_class` varchar(64) DEFAULT NULL,
  `physicalEntity` int unsigned DEFAULT NULL,
  `physicalEntity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `cell` (`cell`),
  KEY `physicalEntity` (`physicalEntity`)
);

CREATE TABLE `PhysicalEntity_2_cellType` (
  `DB_ID` int unsigned DEFAULT NULL,
  `cellType_rank` int unsigned DEFAULT NULL,
  `cellType` int unsigned DEFAULT NULL,
  `cellType_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `cellType` (`cellType`)
);

CREATE TABLE `PhysicalEntity_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `PhysicalEntity_2_disease` (
  `DB_ID` int unsigned DEFAULT NULL,
  `disease_rank` int unsigned DEFAULT NULL,
  `disease` int unsigned DEFAULT NULL,
  `disease_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `disease` (`disease`)
);

CREATE TABLE `PhysicalEntity_2_edited` (
  `DB_ID` int unsigned DEFAULT NULL,
  `edited_rank` int unsigned DEFAULT NULL,
  `edited` int unsigned DEFAULT NULL,
  `edited_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `edited` (`edited`)
);

CREATE TABLE `PhysicalEntity_2_figure` (
  `DB_ID` int unsigned DEFAULT NULL,
  `figure_rank` int unsigned DEFAULT NULL,
  `figure` int unsigned DEFAULT NULL,
  `figure_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `figure` (`figure`)
);

CREATE TABLE `PhysicalEntity_2_inferredFrom` (
  `DB_ID` int unsigned DEFAULT NULL,
  `inferredFrom_rank` int unsigned DEFAULT NULL,
  `inferredFrom` int unsigned DEFAULT NULL,
  `inferredFrom_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `inferredFrom` (`inferredFrom`)
);

CREATE TABLE `PhysicalEntity_2_inferredTo` (
  `DB_ID` int unsigned DEFAULT NULL,
  `inferredTo_rank` int unsigned DEFAULT NULL,
  `inferredTo` int unsigned DEFAULT NULL,
  `inferredTo_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `inferredTo` (`inferredTo`)
);

CREATE TABLE `PhysicalEntity_2_literatureReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `literatureReference_rank` int unsigned DEFAULT NULL,
  `literatureReference` int unsigned DEFAULT NULL,
  `literatureReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `literatureReference` (`literatureReference`)
);

CREATE TABLE `PhysicalEntity_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `PhysicalEntity_2_reviewed` (
  `DB_ID` int unsigned DEFAULT NULL,
  `reviewed_rank` int unsigned DEFAULT NULL,
  `reviewed` int unsigned DEFAULT NULL,
  `reviewed_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `reviewed` (`reviewed`)
);

CREATE TABLE `PhysicalEntity_2_revised` (
  `DB_ID` int unsigned DEFAULT NULL,
  `revised_rank` int unsigned DEFAULT NULL,
  `revised` int unsigned DEFAULT NULL,
  `revised_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `revised` (`revised`)
);

CREATE TABLE `PhysicalEntity_2_summation` (
  `DB_ID` int unsigned DEFAULT NULL,
  `summation_rank` int unsigned DEFAULT NULL,
  `summation` int unsigned DEFAULT NULL,
  `summation_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `summation` (`summation`)
);

CREATE TABLE `Polymer` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `maxUnitCount` int DEFAULT NULL,
  `minUnitCount` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `maxUnitCount` (`maxUnitCount`),
  KEY `minUnitCount` (`minUnitCount`)
);

CREATE TABLE `Polymer_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `Polymer_2_repeatedUnit` (
  `DB_ID` int unsigned DEFAULT NULL,
  `repeatedUnit_rank` int unsigned DEFAULT NULL,
  `repeatedUnit` int unsigned DEFAULT NULL,
  `repeatedUnit_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `repeatedUnit` (`repeatedUnit`)
);

CREATE TABLE `Polymer_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `Polymerisation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `PositiveGeneExpressionRegulation` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `PositiveRegulation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ProteinDrug` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `PsiMod` (
  `DB_ID` int unsigned NOT NULL,
  `label` text,
  PRIMARY KEY (`DB_ID`),
  KEY `label` (`label`(10))
);

CREATE TABLE `Publication` (
  `DB_ID` int unsigned NOT NULL,
  `title` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `title` (`title`(10))
);

CREATE TABLE `Publication_2_author` (
  `DB_ID` int unsigned DEFAULT NULL,
  `author_rank` int unsigned DEFAULT NULL,
  `author` int unsigned DEFAULT NULL,
  `author_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `author` (`author`)
);

CREATE TABLE `RNADrug` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `Reaction` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `reverseReaction` int unsigned DEFAULT NULL,
  `reverseReaction_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `reverseReaction` (`reverseReaction`)
);

CREATE TABLE `ReactionType` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ReactionlikeEvent` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `isChimeric` enum('TRUE','FALSE') DEFAULT NULL,
  `systematicName` mediumtext,
  `normalReaction` int unsigned DEFAULT NULL,
  `normalReaction_class` varchar(64) DEFAULT NULL,
  `catalystActivityReference` int unsigned DEFAULT NULL,
  `catalystActivityReference_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `isChimeric` (`isChimeric`),
  KEY `systematicName` (`systematicName`(10)),
  KEY `normalReaction` (`normalReaction`),
  KEY `catalystActivityReference` (`catalystActivityReference`)
);

CREATE TABLE `ReactionlikeEvent_2_catalystActivity` (
  `DB_ID` int unsigned DEFAULT NULL,
  `catalystActivity_rank` int unsigned DEFAULT NULL,
  `catalystActivity` int unsigned DEFAULT NULL,
  `catalystActivity_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `catalystActivity` (`catalystActivity`)
);

CREATE TABLE `ReactionlikeEvent_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `ReactionlikeEvent_2_entityFunctionalStatus` (
  `DB_ID` int unsigned DEFAULT NULL,
  `entityFunctionalStatus_rank` int unsigned DEFAULT NULL,
  `entityFunctionalStatus` int unsigned DEFAULT NULL,
  `entityFunctionalStatus_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `entityFunctionalStatus` (`entityFunctionalStatus`)
);

CREATE TABLE `ReactionlikeEvent_2_entityOnOtherCell` (
  `DB_ID` int unsigned DEFAULT NULL,
  `entityOnOtherCell_rank` int unsigned DEFAULT NULL,
  `entityOnOtherCell` int unsigned DEFAULT NULL,
  `entityOnOtherCell_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `entityOnOtherCell` (`entityOnOtherCell`)
);

CREATE TABLE `ReactionlikeEvent_2_hasInteraction` (
  `DB_ID` int unsigned DEFAULT NULL,
  `hasInteraction_rank` int unsigned DEFAULT NULL,
  `hasInteraction` int unsigned DEFAULT NULL,
  `hasInteraction_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `hasInteraction` (`hasInteraction`)
);

CREATE TABLE `ReactionlikeEvent_2_input` (
  `DB_ID` int unsigned DEFAULT NULL,
  `input_rank` int unsigned DEFAULT NULL,
  `input` int unsigned DEFAULT NULL,
  `input_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `input` (`input`)
);

CREATE TABLE `ReactionlikeEvent_2_output` (
  `DB_ID` int unsigned DEFAULT NULL,
  `output_rank` int unsigned DEFAULT NULL,
  `output` int unsigned DEFAULT NULL,
  `output_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `output` (`output`)
);

CREATE TABLE `ReactionlikeEvent_2_participantCellType` (
  `DB_ID` int unsigned DEFAULT NULL,
  `participantCellType_rank` int unsigned DEFAULT NULL,
  `participantCellType` int unsigned DEFAULT NULL,
  `participantCellType_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `participantCellType` (`participantCellType`)
);

CREATE TABLE `ReactionlikeEvent_2_reactionType` (
  `DB_ID` int unsigned DEFAULT NULL,
  `reactionType_rank` int unsigned DEFAULT NULL,
  `reactionType` int unsigned DEFAULT NULL,
  `reactionType_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `reactionType` (`reactionType`)
);

CREATE TABLE `ReactionlikeEvent_2_regulatedBy` (
  `DB_ID` int unsigned DEFAULT NULL,
  `regulatedBy_rank` int unsigned DEFAULT NULL,
  `regulatedBy` int unsigned DEFAULT NULL,
  `regulatedBy_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `regulatedBy` (`regulatedBy`)
);

CREATE TABLE `ReactionlikeEvent_2_regulationReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `regulationReference_rank` int unsigned DEFAULT NULL,
  `regulationReference` int unsigned DEFAULT NULL,
  `regulationReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `regulationReference` (`regulationReference`)
);

CREATE TABLE `ReactionlikeEvent_2_requiredInputComponent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `requiredInputComponent_rank` int unsigned DEFAULT NULL,
  `requiredInputComponent` int unsigned DEFAULT NULL,
  `requiredInputComponent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `requiredInputComponent` (`requiredInputComponent`)
);

CREATE TABLE `ReferenceDNASequence` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ReferenceDatabase` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `accessUrl` mediumtext,
  `url` mediumtext,
  `resourceIdentifier` text,
  `identifiersPrefix` text,
  PRIMARY KEY (`DB_ID`),
  KEY `url` (`url`(10)),
  KEY `accessUrl` (`accessUrl`(10)),
  KEY `resourceIdentifier` (`resourceIdentifier`(10)),
  KEY `identifiersPrefix` (`identifiersPrefix`(10))
);

CREATE TABLE `ReferenceDatabase_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `ReferenceEntity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `identifier` mediumtext,
  `referenceDatabase` int unsigned DEFAULT NULL,
  `referenceDatabase_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `referenceDatabase` (`referenceDatabase`),
  KEY `identifier` (`identifier`(10))
);

CREATE TABLE `ReferenceEntity_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `ReferenceEntity_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`(10))
);

CREATE TABLE `ReferenceEntity_2_otherIdentifier` (
  `DB_ID` int unsigned DEFAULT NULL,
  `otherIdentifier_rank` int unsigned DEFAULT NULL,
  `otherIdentifier` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `otherIdentifier` (`otherIdentifier`(10))
);

CREATE TABLE `ReferenceGeneProduct` (
  `DB_ID` int unsigned NOT NULL,
  `_chainChangeLog` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `_chainChangeLog` (`_chainChangeLog`(10))
);

CREATE TABLE `ReferenceGeneProduct_2_chain` (
  `DB_ID` int unsigned DEFAULT NULL,
  `chain_rank` int unsigned DEFAULT NULL,
  `chain` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `chain` (`chain`(10))
);

CREATE TABLE `ReferenceGeneProduct_2_referenceGene` (
  `DB_ID` int unsigned DEFAULT NULL,
  `referenceGene_rank` int unsigned DEFAULT NULL,
  `referenceGene` int unsigned DEFAULT NULL,
  `referenceGene_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `referenceGene` (`referenceGene`)
);

CREATE TABLE `ReferenceGeneProduct_2_referenceTranscript` (
  `DB_ID` int unsigned DEFAULT NULL,
  `referenceTranscript_rank` int unsigned DEFAULT NULL,
  `referenceTranscript` int unsigned DEFAULT NULL,
  `referenceTranscript_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `referenceTranscript` (`referenceTranscript`)
);

CREATE TABLE `ReferenceGroup` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `formula` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `formula` (`formula`(10))
);

CREATE TABLE `ReferenceIsoform` (
  `DB_ID` int unsigned NOT NULL,
  `variantIdentifier` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `variantIdentifier` (`variantIdentifier`(10))
);

CREATE TABLE `ReferenceIsoform_2_isoformParent` (
  `DB_ID` int unsigned DEFAULT NULL,
  `isoformParent_rank` int unsigned DEFAULT NULL,
  `isoformParent` int unsigned DEFAULT NULL,
  `isoformParent_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `isoformParent` (`isoformParent`)
);

CREATE TABLE `ReferenceMolecule` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `formula` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `formula` (`formula`(10))
);

CREATE TABLE `ReferenceRNASequence` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ReferenceRNASequence_2_referenceGene` (
  `DB_ID` int unsigned DEFAULT NULL,
  `referenceGene_rank` int unsigned DEFAULT NULL,
  `referenceGene` int unsigned DEFAULT NULL,
  `referenceGene_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `referenceGene` (`referenceGene`)
);

CREATE TABLE `ReferenceSequence` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  `sequenceLength` int DEFAULT NULL,
  `isSequenceChanged` mediumtext,
  `checksum` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `species` (`species`),
  KEY `sequenceLength` (`sequenceLength`),
  KEY `isSequenceChanged` (`isSequenceChanged`(10)),
  KEY `checksum` (`checksum`(10))
);

CREATE TABLE `ReferenceSequence_2_comment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `comment_rank` int unsigned DEFAULT NULL,
  `comment` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `comment` (`comment`(10))
);

CREATE TABLE `ReferenceSequence_2_description` (
  `DB_ID` int unsigned DEFAULT NULL,
  `description_rank` int unsigned DEFAULT NULL,
  `description` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `description` (`description`(10))
);

CREATE TABLE `ReferenceSequence_2_geneName` (
  `DB_ID` int unsigned DEFAULT NULL,
  `geneName_rank` int unsigned DEFAULT NULL,
  `geneName` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `geneName` (`geneName`(10))
);

CREATE TABLE `ReferenceSequence_2_keyword` (
  `DB_ID` int unsigned DEFAULT NULL,
  `keyword_rank` int unsigned DEFAULT NULL,
  `keyword` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `keyword` (`keyword`(10))
);

CREATE TABLE `ReferenceSequence_2_secondaryIdentifier` (
  `DB_ID` int unsigned DEFAULT NULL,
  `secondaryIdentifier_rank` int unsigned DEFAULT NULL,
  `secondaryIdentifier` mediumtext,
  KEY `DB_ID` (`DB_ID`),
  KEY `secondaryIdentifier` (`secondaryIdentifier`(10))
);

CREATE TABLE `ReferenceTherapeutic` (
  `DB_ID` int unsigned NOT NULL,
  `abbreviation` text,
  `approved` enum('TRUE','FALSE') DEFAULT NULL,
  `inn` text,
  `type` text,
  `withdrawn` enum('TRUE','FALSE') DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `abbreviation` (`abbreviation`(10)),
  KEY `approved` (`approved`),
  KEY `inn` (`inn`(10)),
  KEY `type` (`type`(10)),
  KEY `withdrawn` (`withdrawn`)
);

CREATE TABLE `ReferenceTherapeutic_2_activeDrugIds` (
  `DB_ID` int unsigned DEFAULT NULL,
  `activeDrugIds_rank` int unsigned DEFAULT NULL,
  `activeDrugIds` text,
  KEY `DB_ID` (`DB_ID`),
  KEY `activeDrugIds` (`activeDrugIds`(10))
);

CREATE TABLE `ReferenceTherapeutic_2_approvalSource` (
  `DB_ID` int unsigned DEFAULT NULL,
  `approvalSource_rank` int unsigned DEFAULT NULL,
  `approvalSource` text,
  KEY `DB_ID` (`DB_ID`),
  KEY `approvalSource` (`approvalSource`(10))
);

CREATE TABLE `ReferenceTherapeutic_2_prodrugIds` (
  `DB_ID` int unsigned DEFAULT NULL,
  `prodrugIds_rank` int unsigned DEFAULT NULL,
  `prodrugIds` text,
  KEY `DB_ID` (`DB_ID`),
  KEY `prodrugIds` (`prodrugIds`(10))
);

CREATE TABLE `Regulation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `regulator` int unsigned DEFAULT NULL,
  `regulator_class` varchar(64) DEFAULT NULL,
  `activity` int unsigned DEFAULT NULL,
  `activity_class` varchar(64) DEFAULT NULL,
  `goBiologicalProcess` int unsigned DEFAULT NULL,
  `goBiologicalProcess_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `regulator` (`regulator`),
  KEY `activity` (`activity`),
  KEY `goBiologicalProcess` (`goBiologicalProcess`)
);

CREATE TABLE `RegulationReference` (
  `DB_ID` int unsigned NOT NULL,
  `regulation` int unsigned DEFAULT NULL,
  `regulation_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `regulation` (`regulation`)
);

CREATE TABLE `Regulation_2_activeUnit` (
  `DB_ID` int unsigned DEFAULT NULL,
  `activeUnit_rank` int unsigned DEFAULT NULL,
  `activeUnit` int unsigned DEFAULT NULL,
  `activeUnit_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `activeUnit` (`activeUnit`)
);

CREATE TABLE `ReplacedResidue` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `coordinate` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `coordinate` (`coordinate`)
);

CREATE TABLE `ReplacedResidue_2_psiMod` (
  `DB_ID` int unsigned DEFAULT NULL,
  `psiMod_rank` int unsigned DEFAULT NULL,
  `psiMod` int unsigned DEFAULT NULL,
  `psiMod_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `psiMod` (`psiMod`)
);

CREATE TABLE `Requirement` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `RetractionStatus` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `ReviewStatus` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `SequenceOntology` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `SimpleEntity` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  `referenceEntity` int unsigned DEFAULT NULL,
  `referenceEntity_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `species` (`species`),
  KEY `referenceEntity` (`referenceEntity`)
);

CREATE TABLE `SimpleEntity_2_compartment` (
  `DB_ID` int unsigned DEFAULT NULL,
  `compartment_rank` int unsigned DEFAULT NULL,
  `compartment` int unsigned DEFAULT NULL,
  `compartment_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `compartment` (`compartment`)
);

CREATE TABLE `Species` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `abbreviation` text,
  PRIMARY KEY (`DB_ID`),
  KEY `abbreviation` (`abbreviation`(10))
);

CREATE TABLE `StableIdentifier` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `identifier` mediumtext,
  `identifierVersion` mediumtext,
  `oldIdentifier` mediumtext,
  `oldIdentifierVersion` mediumtext,
  `released` enum('TRUE','FALSE') DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `identifier` (`identifier`(10)),
  KEY `identifierVersion` (`identifierVersion`(10)),
  KEY `oldIdentifierVersion` (`oldIdentifierVersion`(10)),
  KEY `oldIdentifier` (`oldIdentifier`(10)),
  KEY `released` (`released`)
);

CREATE TABLE `StableIdentifierHistory` (
  `DB_ID` int unsigned NOT NULL,
  `identifier` text,
  `identifierVersion` text,
  PRIMARY KEY (`DB_ID`),
  KEY `identifier` (`identifier`(10)),
  KEY `identifierVersion` (`identifierVersion`(10))
);

CREATE TABLE `StableIdentifierHistory_2_historyStatus` (
  `DB_ID` int unsigned DEFAULT NULL,
  `historyStatus_rank` int unsigned DEFAULT NULL,
  `historyStatus` int unsigned DEFAULT NULL,
  `historyStatus_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `historyStatus` (`historyStatus`)
);

CREATE TABLE `StableIdentifierReleaseStatus` (
  `DB_ID` int unsigned NOT NULL,
  `releaseNumber` int DEFAULT NULL,
  `status` text,
  PRIMARY KEY (`DB_ID`),
  KEY `releaseNumber` (`releaseNumber`),
  KEY `status` (`status`(10))
);

CREATE TABLE `StableIdentifier_2_history` (
  `DB_ID` int unsigned DEFAULT NULL,
  `history_rank` int unsigned DEFAULT NULL,
  `history` int unsigned DEFAULT NULL,
  `history_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `history` (`history`)
);

CREATE TABLE `Summation` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `text` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `text` (`text`(10))
);

CREATE TABLE `Summation_2_literatureReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `literatureReference_rank` int unsigned DEFAULT NULL,
  `literatureReference` int unsigned DEFAULT NULL,
  `literatureReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `literatureReference` (`literatureReference`)
);

CREATE TABLE `Taxon` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `superTaxon` int unsigned DEFAULT NULL,
  `superTaxon_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `superTaxon` (`superTaxon`)
);

CREATE TABLE `Taxon_2_crossReference` (
  `DB_ID` int unsigned DEFAULT NULL,
  `crossReference_rank` int unsigned DEFAULT NULL,
  `crossReference` int unsigned DEFAULT NULL,
  `crossReference_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `crossReference` (`crossReference`)
);

CREATE TABLE `Taxon_2_name` (
  `DB_ID` int unsigned DEFAULT NULL,
  `name_rank` int unsigned DEFAULT NULL,
  `name` varchar(255) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `name` (`name`)
);

CREATE TABLE `TranscriptionalModification` (
  `DB_ID` int unsigned NOT NULL,
  PRIMARY KEY (`DB_ID`)
);

CREATE TABLE `TranslationalModification` (
  `DB_ID` int unsigned NOT NULL,
  `coordinate` int DEFAULT NULL,
  `psiMod` int unsigned DEFAULT NULL,
  `psiMod_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `coordinate` (`coordinate`),
  KEY `psiMod` (`psiMod`)
);

CREATE TABLE `URL` (
  `DB_ID` int unsigned NOT NULL,
  `uniformResourceLocator` mediumtext,
  PRIMARY KEY (`DB_ID`),
  KEY `uniformResourceLocator` (`uniformResourceLocator`(10))
);

CREATE TABLE `_Deleted` (
  `DB_ID` int unsigned NOT NULL DEFAULT '0',
  `curatorComment` mediumtext,
  `reason` int unsigned DEFAULT NULL,
  `reason_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `curatorComment` (`curatorComment`(10)),
  KEY `reason` (`reason`)
);

CREATE TABLE `_DeletedInstance` (
  `DB_ID` int unsigned NOT NULL,
  `class` text,
  `deletedInstanceDB_ID` int DEFAULT NULL,
  `deletedStableIdentifier` int unsigned DEFAULT NULL,
  `deletedStableIdentifier_class` varchar(64) DEFAULT NULL,
  `name` text,
  PRIMARY KEY (`DB_ID`),
  KEY `class` (`class`(10)),
  KEY `deletedInstanceDB_ID` (`deletedInstanceDB_ID`),
  KEY `deletedStableIdentifier` (`deletedStableIdentifier`),
  KEY `name` (`name`(10))
);

CREATE TABLE `_DeletedInstance_2_species` (
  `DB_ID` int unsigned DEFAULT NULL,
  `species_rank` int unsigned DEFAULT NULL,
  `species` int unsigned DEFAULT NULL,
  `species_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `species` (`species`)
);

CREATE TABLE `_Deleted_2_deletedInstance` (
  `DB_ID` int unsigned DEFAULT NULL,
  `deletedInstance_rank` int unsigned DEFAULT NULL,
  `deletedInstance` int unsigned DEFAULT NULL,
  `deletedInstance_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `deletedInstance` (`deletedInstance`)
);

CREATE TABLE `_Deleted_2_deletedInstanceDB_ID` (
  `DB_ID` int unsigned DEFAULT NULL,
  `deletedInstanceDB_ID_rank` int unsigned DEFAULT NULL,
  `deletedInstanceDB_ID` int DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `deletedInstanceDB_ID` (`deletedInstanceDB_ID`)
);

CREATE TABLE `_Deleted_2_replacementInstanceDB_IDs` (
  `DB_ID` int unsigned DEFAULT NULL,
  `replacementInstanceDB_IDs_rank` int unsigned DEFAULT NULL,
  `replacementInstanceDB_IDs` int DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `replacementInstanceDB_IDs` (`replacementInstanceDB_IDs`)
);

CREATE TABLE `_Deleted_2_replacementInstances` (
  `DB_ID` int unsigned DEFAULT NULL,
  `replacementInstances_rank` int unsigned DEFAULT NULL,
  `replacementInstances` int unsigned DEFAULT NULL,
  `replacementInstances_class` varchar(64) DEFAULT NULL,
  KEY `DB_ID` (`DB_ID`),
  KEY `replacementInstances` (`replacementInstances`)
);

CREATE TABLE `_Release` (
  `DB_ID` int unsigned NOT NULL,
  `releaseDate` mediumtext,
  `releaseNumber` int DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `releaseDate` (`releaseDate`(10)),
  KEY `releaseNumber` (`releaseNumber`)
);

CREATE TABLE `_UpdateTracker` (
  `DB_ID` int unsigned NOT NULL,
  `_release` int unsigned DEFAULT NULL,
  `_release_class` varchar(64) DEFAULT NULL,
  `updatedInstance` int unsigned DEFAULT NULL,
  `updatedInstance_class` varchar(64) DEFAULT NULL,
  PRIMARY KEY (`DB_ID`),
  KEY `_release` (`_release`),
  KEY `updatedInstance` (`updatedInstance`)
);

CREATE TABLE `_UpdateTracker_2_action` (
  `DB_ID` int unsigned DEFAULT NULL,
  `action_rank` int unsigned DEFAULT NULL,
  `action` text,
  KEY `DB_ID` (`DB_ID`),
  KEY `action` (`action`(10))
);
