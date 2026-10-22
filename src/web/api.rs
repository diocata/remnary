use super::{ApiError, ApiResult, MAX_BODY_BYTES, error, header, json_response};
use crate::{analysis_store::FeedbackAction, store::Store};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Cursor, Read};
use tiny_http::{Method, Request, Response};

pub(super) fn api(
    request: &mut Request,
    store: &mut Store,
    url: &Url,
) -> ApiResult<Response<Cursor<Vec<u8>>>> {
    match (request.method().clone(), url.path()) {
        (Method::Get, "/api/status") => {
            no_query(url)?;
            let stats = store.stats().map_err(|_| database_error())?;
            let observations = store.observations(false).map_err(|_| database_error())?;
            Ok(json_response(
                200,
                json!({"conversations":stats.conversations,"messages":stats.messages,"revisions":stats.revisions,"observations":observations.len()}),
            ))
        }
        (Method::Get, "/api/observations") => {
            let hidden = optional_query(url, "hidden")?;
            if hidden
                .as_deref()
                .is_some_and(|value| value != "1" && value != "0")
            {
                return Err(error(400, "The hidden option must be 0 or 1."));
            }
            let observations = store
                .observations(hidden.as_deref() == Some("1"))
                .map_err(|_| database_error())?;
            let (visible, hidden): (Vec<_>, Vec<_>) = observations
                .into_iter()
                .partition(|r| r.status.is_visible());
            let topics = store
                .topic_threads_from(&visible)
                .map_err(|_| database_error())?;
            let mut observations = crate::reports::rank_discoveries(visible, usize::MAX);
            observations.extend(hidden);
            Ok(json_response(
                200,
                json!({"observations":observations,"topics":topics}),
            ))
        }
        (Method::Get, "/api/search") => {
            let query = optional_query(url, "q")?.unwrap_or_default();
            if query.trim().is_empty() || query.len() > 2048 {
                return Err(error(400, "Enter a search query of 1 to 2048 bytes."));
            }
            let hits = store.search(&query, 100).map_err(|_| database_error())?;
            let hits: Vec<_> = hits.into_iter().map(|hit| json!({
                "provider":hit.provider,"conversation_id":crate::model::digest(&serde_json::to_vec(&(&hit.provider, &hit.conversation_id)).expect("string tuples serialize")),"title":hit.title,"revision":hit.revision,
                "message_id":hit.message_id,"role":hit.role,"excerpt":hit.excerpt
            })).collect();
            Ok(json_response(200, json!({"hits":hits})))
        }
        (Method::Get, "/api/sources") => {
            no_query(url)?;
            let sources = store.current_snapshots().map_err(|_| database_error())?;
            let sources: Vec<_> = sources.into_iter().map(|target| {
                let source = target.conversation;
                json!({"id":source.stable_id(),"provider":source.provider,"title":source.title,
                    "messages":source.messages.len(),"created_at":source.created_at,"updated_at":source.updated_at,"revision":target.revision,
                    "partial_view":crate::app_snapshot::is_partial(&source.metadata)})
            }).collect();
            Ok(json_response(200, json!({"sources":sources})))
        }
        (Method::Get, "/api/source") => {
            let mut id = None;
            let mut revision = None;
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "conversation_id" if id.is_none() => id = Some(value.into_owned()),
                    "revision" if revision.is_none() => {
                        let value = value
                            .parse::<i64>()
                            .ok()
                            .filter(|value| *value > 0)
                            .ok_or_else(|| error(400, "Revision must be a positive integer."))?;
                        revision = Some(value);
                    }
                    _ => return Err(error(400, "Unknown or repeated source query parameter.")),
                }
            }
            let id = id
                .filter(|id| valid_id(id))
                .ok_or_else(|| error(400, "A valid conversation ID is required."))?;
            let revision = match revision {
                Some(revision) => revision,
                None => store
                    .current_snapshots()
                    .map_err(|_| database_error())?
                    .into_iter()
                    .find(|target| target.conversation.stable_id() == id)
                    .map(|target| target.revision)
                    .ok_or_else(|| {
                        error(404, "This source revision is unavailable or excluded.")
                    })?,
            };
            let source = store
                .snapshot(&id, Some(revision))
                .map_err(|_| error(404, "This source revision is unavailable or excluded."))?;
            let partial_view = crate::app_snapshot::is_partial(&source.metadata);
            let messages: Vec<_> = source.messages.into_iter().map(|message| json!({
                "source_id":message.source_id,"parent_source_id":message.parent_source_id,"role":message.role,"content":message.content,"created_at":message.created_at,
                "truncated":message.metadata.pointer("/app_item/truncated").and_then(Value::as_bool).unwrap_or(false)
            })).collect();
            Ok(json_response(
                200,
                json!({"id":id,"provider":source.provider,"title":source.title,"revision":revision,"messages":messages,"partial_view":partial_view}),
            ))
        }
        (Method::Post, "/api/feedback") => {
            no_query(url)?;
            let content_type = header(request, "Content-Type").unwrap_or_default();
            if !content_type
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/json")
            {
                return Err(error(
                    415,
                    "Review actions require an application/json body.",
                ));
            }
            if request
                .body_length()
                .is_some_and(|length| length as u64 > MAX_BODY_BYTES)
            {
                return Err(error(413, "Request body exceeds 64 KiB."));
            }
            let mut body = Vec::new();
            request
                .as_reader()
                .take(MAX_BODY_BYTES + 1)
                .read_to_end(&mut body)
                .map_err(|_| error(400, "Could not read the JSON request body."))?;
            if body.len() as u64 > MAX_BODY_BYTES {
                return Err(error(413, "Request body exceeds 64 KiB."));
            }
            let feedback: FeedbackRequest = serde_json::from_slice(&body)
                .map_err(|_| error(400, "Invalid review action. Use an observation ID, action, and optional correction text."))?;
            if !valid_id(&feedback.observation_id)
                || feedback
                    .value
                    .as_ref()
                    .is_some_and(|value| value.len() > 16_384)
                || (matches!(feedback.action, FeedbackAction::Correct)
                    && !feedback
                        .value
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty()))
                || (!matches!(feedback.action, FeedbackAction::Correct) && feedback.value.is_some())
            {
                return Err(error(
                    400,
                    "A correction needs nonempty text of at most 16 KiB; other actions do not accept text.",
                ));
            }
            if !store
                .observations(true)
                .map_err(|_| database_error())?
                .iter()
                .any(|record| record.id == feedback.observation_id)
            {
                return Err(error(
                    404,
                    "This observation is no longer available. Refresh the workspace.",
                ));
            }
            store
                .feedback(
                    &feedback.observation_id,
                    feedback.action,
                    feedback.value.as_deref(),
                )
                .map_err(|_| database_error())?;
            Ok(json_response(200, json!({"ok":true})))
        }
        (_, "/api/feedback") => Err(error(405, "Review actions require POST with a JSON body.")),
        _ => Err(error(404, "API route not found.")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackRequest {
    observation_id: String,
    action: FeedbackAction,
    value: Option<String>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|value| value.is_ascii_hexdigit())
}

fn no_query(url: &Url) -> ApiResult<()> {
    if url.query().is_some() {
        Err(error(400, "This route does not accept query parameters."))
    } else {
        Ok(())
    }
}

fn optional_query(url: &Url, allowed: &str) -> ApiResult<Option<String>> {
    let mut value = None;
    for (key, item) in url.query_pairs() {
        if key != allowed || value.is_some() {
            return Err(error(400, "Unknown or repeated query parameter."));
        }
        value = Some(item.into_owned());
    }
    Ok(value)
}

fn database_error() -> ApiError {
    error(
        500,
        "Could not read or update the local archive. Refresh and try again.",
    )
}
