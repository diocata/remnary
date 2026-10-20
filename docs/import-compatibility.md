# Published ChatGPT record compatibility

Checked 2026-10-20 with the packaged-candidate implementation at commit `aaf0412ccbdf`. The source is [sanand0/openai-conversations](https://github.com/sanand0/openai-conversations/tree/e97c74113ee8b5c650efd8a158d6cbab8abfe4c9/samples), commit `e97c74113ee8b5c650efd8a158d6cbab8abfe4c9`, whose author publishes sample records from their ChatGPT export under MIT. These are another author's deliberately public records, not the owner's private history or a whole-account export ZIP.

Five original sample objects were placed unchanged into an outer conversation array, imported into a separate encrypted archive and reimported. Every original graph/message/metadata record reconstructed exactly from normalized storage. Observed totals: **five conversations, 82 graph nodes, 77 non-null messages**. Roles included system (7), user (17), assistant (30) and tool (23). Content types included text, user-editable context, code, multimodal text, browsing display and quote forms. Non-text parts remain in original message metadata; importing does not imply OCR or interpretation of attachments.

The same real payload imported unchanged when wrapped in a generated ZIP containing `conversations.json` and a generated ZIP containing two numeric shards. Repeat imports produced five unchanged conversations with zero updates. SQLCipher integrity/foreign keys passed. The ZIP wrappers were generated for this check; an original downloaded account ZIP was not observed. No personal extraction/profile analysis was run on these public records, and they are not mixed with the owner's 40-conversation evaluation.

| Published sample basename | Graph nodes | Original file SHA-256 |
| --- | --- | --- |
| `amazon-nova-model-strengths` | 8 | `d33a60710a24472597531989ad7a459f445c64979ff7b5bc9fd02adb1898fd9f` |
| `csv-data-analysis-insights` | 6 | `98b54f72b998eb4b019ea3cf4f0843ffb3f7912574edd183ccf245c06c478628` |
| `india-map-with-khargone` | 48 | `35547d1ee4cd1e3cc3be9d6119e4bafcf5a64a3ab3dbcc4d34c7e0d671d96bcf` |
| `node-js-network-libraries` | 8 | `60d3c01050e232f6173130b8b10c4296b2f0bd29a83fba8a4be4522d972e6370` |
| `seoul-weather-early-october` | 12 | `811bf0956926e0596bc31b06380954f88bcbefdb7fa202de8d0ef22f74f5b9e6` |

Only provenance and observed results are committed; downloaded records, generated wrappers and the separate encrypted check archive are ignored local evaluation artifacts. The original sample bytes were fetched from immutable commit URLs. This verifies these published mapping records and the tested container contracts, not every current or future export layout. [Supported formats and limits](sources.md).

A scrubbed copy of the published weather record is now a permanent regression fixture at `tests/fixtures/upstream-chatgpt-weather.json`, with its MIT notice and [scrubbing provenance](../tests/fixtures/upstream-chatgpt-PROVENANCE.md). Its test checks all 12 graph nodes and 11 messages and exact reconstruction of the scrubbed record. Original published text is not committed.
