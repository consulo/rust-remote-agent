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

use std::process::Command;
use std::path::Path;

fn main() {
    let thrift_file = "thrift/remote_agent.thrift";
    let out_dir = "src/generated";

    println!("cargo:rerun-if-changed={}", thrift_file);

    if !Path::new(thrift_file).exists() {
        panic!("Thrift IDL file not found: {}", thrift_file);
    }

    // Try to run thrift compiler
    let status = Command::new("thrift")
        .args([
            "--gen", "rs",
            "-out", out_dir,
            thrift_file,
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=Thrift code generated successfully");
        }
        Ok(s) => {
            println!(
                "cargo:warning=Thrift compiler exited with: {}. Using pre-generated code.",
                s
            );
        }
        Err(e) => {
            println!(
                "cargo:warning=Thrift compiler not found ({}). Using pre-generated code.",
                e
            );
        }
    }
}
