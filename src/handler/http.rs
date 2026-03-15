// Copyright 2013-2026 consulo.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::BTreeMap;
use crate::generated::remote_agent::{AgentException, HttpRequest, HttpResponse};

fn agent_err(msg: String) -> thrift::Error {
    thrift::Error::User(Box::new(AgentException::new(msg)))
}

fn read_response(response: ureq::http::Response<ureq::Body>) -> thrift::Result<HttpResponse> {
    let status_code = response.status().as_u16() as i32;

    let mut response_headers: BTreeMap<String, String> = BTreeMap::new();
    let headers = response.headers();
    for key in headers.keys() {
        if let Some(value) = headers.get(key) {
            if let Ok(v) = value.to_str() {
                response_headers.insert(key.as_str().to_string(), v.to_string());
            }
        }
    }

    let body = response
        .into_body()
        .read_to_vec()
        .map_err(|e| agent_err(format!("Failed to read HTTP response body: {}", e)))?;

    Ok(HttpResponse::new(
        status_code,
        body,
        if response_headers.is_empty() {
            None
        } else {
            Some(response_headers)
        },
    ))
}

fn handle_error(err: ureq::Error) -> thrift::Result<HttpResponse> {
    match err {
        ureq::Error::StatusCode(code) => Ok(HttpResponse::new(code as i32, Vec::new(), None)),
        e => Err(agent_err(format!("HTTP request failed: {}", e))),
    }
}

pub fn execute_http_request(request: HttpRequest) -> thrift::Result<HttpResponse> {
    let method = request.method.to_uppercase();
    let agent = ureq::Agent::new_with_defaults();

    match method.as_str() {
        "GET" => {
            let mut req = agent.get(&request.url);
            if let Some(headers) = &request.headers {
                for (key, value) in headers {
                    req = req.header(key.as_str(), value.as_str());
                }
            }
            match req.call() {
                Ok(resp) => read_response(resp),
                Err(e) => handle_error(e),
            }
        }
        "DELETE" => {
            let mut req = agent.delete(&request.url);
            if let Some(headers) = &request.headers {
                for (key, value) in headers {
                    req = req.header(key.as_str(), value.as_str());
                }
            }
            match req.call() {
                Ok(resp) => read_response(resp),
                Err(e) => handle_error(e),
            }
        }
        "POST" => {
            let mut req = agent.post(&request.url);
            if let Some(headers) = &request.headers {
                for (key, value) in headers {
                    req = req.header(key.as_str(), value.as_str());
                }
            }
            let body_bytes = request.body.as_deref().unwrap_or(&[]);
            match req.send(body_bytes) {
                Ok(resp) => read_response(resp),
                Err(e) => handle_error(e),
            }
        }
        _ => Err(agent_err(format!(
            "Unsupported HTTP method: {}",
            request.method
        ))),
    }
}
