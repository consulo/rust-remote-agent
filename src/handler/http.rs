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

use std::collections::HashMap;
use std::time::Duration;

use ureq::ResponseExt;

use crate::error::{AgentError, AgentResult, agent_err};
use crate::proto::{HttpRequest, HttpResponse};

fn millis(value: Option<i32>) -> Option<Duration> {
    value
        .filter(|v| *v > 0)
        .map(|v| Duration::from_millis(v as u64))
}

fn create_agent(request: &HttpRequest) -> ureq::Agent {
    let read_timeout = millis(request.read_timeout_millis);
    let mut config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(millis(request.connect_timeout_millis))
        .timeout_recv_response(read_timeout)
        .timeout_recv_body(read_timeout);
    if let Some(max_redirects) = request.max_redirects {
        config = config
            .max_redirects(max_redirects.max(0) as u32)
            .max_redirects_will_error(false);
    }
    config.build().into()
}

fn read_response(response: ureq::http::Response<ureq::Body>) -> AgentResult<HttpResponse> {
    let status_code = response.status().as_u16() as i32;
    let url = response.get_uri().to_string();

    let mut response_headers: HashMap<String, String> = HashMap::new();
    let headers = response.headers();
    for key in headers.keys() {
        let values: Vec<&str> = headers
            .get_all(key)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        if !values.is_empty() {
            response_headers.insert(key.as_str().to_string(), values.join(", "));
        }
    }

    let body = response
        .into_body()
        .read_to_vec()
        .map_err(|e| agent_err(format!("Failed to read HTTP response body: {}", e)))?;

    Ok(HttpResponse {
        status_code,
        body,
        headers: response_headers,
        url,
    })
}

pub fn execute_http_request(request: HttpRequest) -> AgentResult<HttpResponse> {
    let method = ureq::http::Method::from_bytes(request.method.to_uppercase().as_bytes())
        .map_err(|e| AgentError::InvalidArgument(format!("Unsupported HTTP method '{}': {}", request.method, e)))?;

    let agent = create_agent(&request);

    let mut builder = ureq::http::Request::builder().method(method).uri(&request.url);
    for (key, value) in &request.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    let result = match request.body {
        Some(body) => builder
            .body(body)
            .map_err(|e| AgentError::InvalidArgument(format!("Invalid HTTP request: {}", e)))
            .map(|http_request| agent.run(http_request))?,
        None => builder
            .body(())
            .map_err(|e| AgentError::InvalidArgument(format!("Invalid HTTP request: {}", e)))
            .map(|http_request| agent.run(http_request))?,
    };

    let response = result.map_err(|e| agent_err(format!("HTTP request failed: {}", e)))?;
    read_response(response)
}
