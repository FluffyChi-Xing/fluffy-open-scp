# Recorded probe fixtures

Extracted from existing local captures, not generated mock probe layouts:

- resource.json: package-runtime-data-20261017, sequences 3, 4, 61–64. Paths replaced with `fixture/<basename>`.
- ui.json: ui-native-exhaustive-build-20261014, sequences 1–4 and 90. Includes actual tagged JS values and UnitRuleUIEvent payload.
- engine.json: engine-logs-server-20261017, sequence 2. Original Config ArgScript output, no configuration values.
- global-event.json: ui-native-exhaustive-build-20261014, sequence 40090. Actual global UI event (248287250), without unit identity or rule-success flag.

Tests mutate copies for negative pairing and value-change cases. Full captures stay local in tmp.
