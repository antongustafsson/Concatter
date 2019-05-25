use serde_json;
use reqwest;

pub struct JSPathCollection {
    pub component_name: String,
    pub component_version: String,
    pub paths: Vec<String>,
}

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

pub fn get_component_js_paths(diversity_json_string: &String) -> Option<Vec<String>> {
    let json_parse_result: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(&diversity_json_string);
    match json_parse_result {
        Ok(diversity_json_value) => {
            match diversity_json_value.get("script") {
                Some(script_value) => {
                    match script_value.as_array() {
                        Some(script_array) => {
                            let mut script_paths: Vec<String> = vec![];
                            for possible_string in script_array.into_iter() {
                                match possible_string.as_str() {
                                    Some(value) => script_paths.push(String::from(value)),
                                    None => {
                                        println!("Script array contained invalid type");
                                    }
                                }
                            }
                            match script_paths.len() > 0 {
                                true => Some(script_paths),
                                false => None
                            }
                        },
                        None => {
                            match script_value.as_str() {
                                Some(script_string) => {
                                    Some(vec![String::from(script_string)])
                                },
                                None => {
                                    println!("Script key is invalid in diversity json");
                                    None
                                }
                            }
                        }
                    }
                },
                None => {
                    println!("No script references in diversity json");
                    None
                }
            }
        },
        Err(error) => {
            println!("Failed to parse diversity json: {:?}", error);
            None
        }
    }
}

pub fn request_component_js_paths(ds_api_server: &String, component_name: &String, component_version: &String) -> Option<JSPathCollection> {
    let url = build_component_filepath(&ds_api_server, &component_name, &component_version, "diversity.json");

    fn do_request(url: &String, component_name: &String, component_version: &String, retries: usize) -> Option<JSPathCollection> {
        let request: std::result::Result<String, reqwest::Error> = reqwest::get(url).and_then(|mut response| response.text());

        match request {
            Ok(json_string) => {
                get_component_js_paths(&json_string).and_then(|js_paths| {
                    Some(JSPathCollection {
                        component_name: component_name.clone(),
                        component_version: component_version.clone(),
                        paths: js_paths,
                    })
                })
            },
            Err(_) => {
                if retries < 10 {
                    println!("Failed to load diversity json: ({}), will retry", &url);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    do_request(&url, &component_name, &component_version, retries + 1)
                } else {
                    println!("Failed to load diversity json: ({}), will not retry", &url);
                    None
                }
            }
        }
    }

    do_request(&url, &component_name, &component_version, 0)
}

pub fn get_remote_component_file_contents(ds_api_server: &String, component: &String, version: &String, file: &String) -> Option<String> {
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