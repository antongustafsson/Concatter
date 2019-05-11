use serde_json;
use reqwest;

fn build_component_filepath(ds_api_server: &str, component: &str, version: &str, file: &str) -> String {
    let mut url = String::from("http://");
    url.push_str(ds_api_server);
    url.push_str("/components");
    url.push_str("/");
    url.push_str(&component);
    url.push_str("/");
    url.push_str(&version);
    url.push_str("/files");
    url.push_str("/");
    url.push_str(&file);
    url
}

pub fn get_component_js_paths(ds_api_server: &String, component: &String, version: &String) -> Vec<String> {
    let url = build_component_filepath(&ds_api_server, &component, &version, "diversity.json");
    let request: std::result::Result<String, reqwest::Error> = reqwest::get(&url).and_then(|mut response| response.text());
    let mut script_files: Vec<String> = Vec::new();

    match request {
        Ok(json_string) => {
            let json_parse_result: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(&json_string);
            match json_parse_result {
                Ok(diversity_json_value) => {
                    match diversity_json_value.get("script") {
                        Some(script_value) => {
                            match script_value.as_array() {
                                Some(script_array) => {
                                    for possible_string in script_array.into_iter() {
                                        match possible_string.as_str() {
                                            Some(value) => script_files.push(String::from(value)),
                                            None => println!("Script array contained invalid type")
                                        }
                                    }
                                },
                                None => {
                                    match script_value.as_str() {
                                        Some(script_string) => {
                                            script_files.push(String::from(script_string));
                                        },
                                        None => println!("Script key is invalid in diversity json")
                                    }
                                }
                            }
                        },
                        None => println!("No script references in diversity json")
                    }
                },
                Err(error) => println!("Failed to parse diversity json: {:?}", error)
            }
        },
        Err(error) => {
            println!("Failed to load diversity json: {:?}", error);
        }
    }
    script_files
}

pub fn get_component_file_contents(ds_api_server: &String, component: &String, version: &String, file: &String) -> Option<String> {
    let url = build_component_filepath(&ds_api_server, &component, &version, &file);
    let request: std::result::Result<String, reqwest::Error> = reqwest::get(&url).and_then(|mut response| response.text());
    match request {
        Ok(content) => {
            Some(content)
        },
        Err(error) => {
            println!("Error when loading file {} for component {}/{}: {:?}", file, component, version, error);
            None
        }
    }
}