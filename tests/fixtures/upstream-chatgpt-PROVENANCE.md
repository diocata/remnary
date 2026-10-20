# Published export structure fixture

`upstream-chatgpt-weather.json` derives from the deliberately public [Seoul weather sample](https://github.com/sanand0/openai-conversations/blob/e97c74113ee8b5c650efd8a158d6cbab8abfe4c9/samples/seoul-weather-early-october.json) in sanand0/openai-conversations, commit `e97c74113ee8b5c650efd8a158d6cbab8abfe4c9`. The original MIT license and copyright notice are preserved in `upstream-chatgpt-LICENSE.txt`.

All nonempty strings except structural identifiers/enums have been replaced by neutral synthetic placeholders. Dictionary/list nesting, field names, nulls, numbers, booleans, node/message IDs and parent/child relationships remain. The single original conversation object is wrapped in an outer array to represent conversations.json. This keeps all 12 graph nodes and 11 non-null messages, including tool content and provider metadata, without republishing the original conversation text.

This is a scrubbed real-format regression fixture, not real personal evidence or an account ZIP. The five original published records were also imported/reconstructed separately in local verification; see [compatibility evidence](../../docs/import-compatibility.md).
