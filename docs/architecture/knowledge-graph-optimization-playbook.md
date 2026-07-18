# Knowledge graph optimization playbook

This document records the reusable rules used by DataMax V3 knowledge graphs. It applies to dataset understanding graphs, cross-dataset graphs, and domain views such as resume analysis.

## 1. Start from the decision, not the canvas

A graph is useful only when a user can answer three questions quickly:

1. What business objects are present?
2. How are they related, and how reliable is each relation?
3. Which source evidence supports the selected node or relation?

The default view should therefore expose a small number of domain facets, a visible-result count, active filters, and a direct path to evidence. Adding more nodes is not an optimization by itself.

## 2. Keep the semantic layer and the evidence layer separate

- Business labels, aliases, domain facets, similarity, and inferred relations are retrieval and navigation hints.
- Confirmed and observed facts must resolve to traceable evidence.
- Inferred-only relations must be visibly marked and must never be presented as citations.
- Evidence should retain opaque identifiers and locators: evidence ID, document ID, chunk ID, and source locator when available.
- Missing provenance must lower the relation to inferred-only instead of fabricating a locator.

## 3. Use progressive disclosure

The default graph should show the dataset, major objects, and a limited set of meaningful fields. Users expand details deliberately.

- Compact: dataset and object themes.
- Standard: representative fields and reliable relations.
- Expanded: additional fields while preserving the prior layout.

Budgets must consider canvas size, label collisions, and edge density, not only raw node counts. Truncated graphs must say that summaries describe the currently visible subgraph.

## 4. Treat business lenses as projections

A lens does not add facts or edges. It selects existing nodes and relations through a reusable configuration:

- observable signals: semantic roles, relation types, resolved labels, source groups;
- included node IDs and relation IDs;
- evidence class and coverage;
- a concise explanation of what can and cannot be concluded.

The fallback lens groups content by object and semantic role. Domain configurations add named facets without replacing the generic graph engine.

### Resume domain configuration

The resume view can expose these facets when matching evidence exists:

- candidate subject;
- employment organization and role;
- work history and time;
- project delivery, responsibilities, and outcomes;
- skills and technical stack;
- education and qualifications;
- location and other contextual dimensions;
- data quality and evidence gaps.

Names, phone numbers, email addresses, and raw example values must not appear in lens summaries or the primary canvas. Shared skills are shared concepts, not proof that two records are the same person. Equal names, companies, or labels must not trigger identity merging.

## 5. Encode relation meaning and confidence independently

- Color and opacity communicate confirmed, observed, and inferred confidence.
- Line style, icon, or endpoint shape communicates structural, reference, identity, and similarity meaning.
- Direction is explicit where direction has business meaning.
- A selected node may reveal adjacent edge labels; the full graph should avoid permanent label noise.

The graph must remain understandable without color alone.

## 6. Keep selection and filters honest

All view controls form one graph-view state: scope, dataset cluster, lens, category, confidence, hop depth, density, and focus mode.

- The UI shows active filters and visible node/relation counts.
- A reset action restores a documented default.
- If a filter hides the selected node or relation, the inspector clears or moves to a visible object.
- Dataset selection used by the workspace and dataset selection used by the graph must not silently diverge.

## 7. Make dense labels distinguishable

Short labels are generated in the context of the visible graph, not by blindly taking the first characters. Collision handling can use the parent object, semantic-role suffix, or distinguishing tail characters. The complete label remains available in the tooltip and inspector.

## 8. Accessibility and mobile behavior are graph features

- Provide a keyboard-navigable node/relation list synchronized with the canvas.
- Focus mode behaves as a modal dialog, traps focus, closes with Escape, and restores focus.
- Mobile defaults to the object/relation/evidence list; the canvas opens on demand.
- Touch targets are at least 44 CSS pixels and evidence details are not displaced by the canvas.

## 9. Test the rules across domains

Every reusable change should be tested against at least one operational dataset and one resume fixture.

Required invariants:

- deterministic output regardless of input ordering;
- selected and reliable endpoints survive budgeting;
- inferred similarity never becomes confirmed identity;
- personal example values do not enter the primary graph;
- evidence identifiers survive the projection;
- missing provenance is visibly downgraded;
- truncation is disclosed;
- label collisions remain distinguishable;
- the mobile view has no horizontal overflow.

## 10. Implementation sequence

1. Stabilize view state, search, selection correction, and visible counts.
2. Add a generic lens registry and evidence inspector.
3. Register domain lenses, starting with resumes, without adding facts.
4. Only then add backend domain relations whose endpoints and provenance are explicit.
5. Promote a relation to confirmed or observed only when the underlying evidence contract supports it.
