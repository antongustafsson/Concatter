extern crate clap;
extern crate rayon;
extern crate reqwest;
use clap::{App, Arg};
use rayon::prelude::*;
use tiny_http;
use std::fs;
use std::path::Path;
mod load;

#[derive(Debug)]
struct QueryParam {
    name: String,
    value: Option<String>,
}

struct LoadedComponent {
    name: String,
    version: String,
    code: String,
}

fn parse_params(url: &String) -> Option<Vec<QueryParam>> {
    match url.split(|c| c == '?').nth(1) {
        Some(params_string) => {
            let query_params: Vec<QueryParam> = params_string
                .split(|c| c == '&')
                .filter_map(|param| {
                    let possible_pair: Vec<String> =
                        param.split('=').map(|part| String::from(part)).collect();

                    match possible_pair.get(0).and_then(|value: &String| {
                        if value.len() > 0 {
                            Some(value)
                        } else {
                            None
                        }
                    }) {
                        Some(value) => {
                            let query_param = QueryParam {
                                name: value.clone(),
                                value: possible_pair.get(1).and_then(|value| Some(value.clone())),
                            };
                            Some(query_param)
                        }
                        None => None,
                    }
                })
                .collect();
            if query_params.len() > 0 {
                Some(query_params)
            } else {
                None
            }
        }
        None => None,
    }
}

fn push_component_code(bundle: &mut String, loaded_component: &LoadedComponent) {
    bundle.push_str("// --- BEGIN ");
    bundle.push_str(&loaded_component.name);
    bundle.push_str(" ---\n");
    bundle.push_str(&loaded_component.code);
    bundle.push_str("\n// --- END ");
    bundle.push_str(&loaded_component.name);
    bundle.push_str(" ---\n\n");
}

fn build_component_cache_key(component_name: &String, component_version: &String) -> String {
    let mut cache_key = String::new();
    cache_key.push_str(&component_name);
    cache_key.push_str(&component_version);
    cache_key
}

fn cache_loaded_component_code(
    code_cache: &chashmap::CHashMap<String, String>,
    component_name: &String,
    component_version: &String,
    component_code: &String,
) {
    let cache_key = build_component_cache_key(&component_name, &component_version);
    code_cache.insert_new(cache_key, component_code.clone());
}

fn get_cached_component_code(
    code_cache: &chashmap::CHashMap<String, String>,
    component_name: &String,
    component_version: &String,
) -> Option<String> {
    let cache_key = build_component_cache_key(&component_name, &component_version);
    match code_cache.get(&cache_key) {
        Some(cache_value_read_guard) => Some(String::from(cache_value_read_guard.as_str())),
        None => None,
    }
}

fn get_remote_loaded_component(ds_api_server: &String, populate_cache: bool, disable_logging: bool, code_cache: &chashmap::CHashMap<String, String>, component_name: &String, component_version: &String, verbose: bool) -> Option<LoadedComponent> {
    match get_cached_component_code(&code_cache, &component_name, &component_version) {
        Some(cached_component_code) => {
            Some(LoadedComponent {
                name: component_name.clone(),
                version: component_version.clone(),
                code: cached_component_code.clone(),
            })
        },
        None => {
            match load::request_component_js_paths(&ds_api_server, &component_name, &component_version) {
                Some(js_paths_collection) => {
                    match remote_js_paths_collection_into_loaded_component(js_paths_collection, &ds_api_server, disable_logging, verbose) {
                        Some(loaded_component) => {
                            if populate_cache {
                                cache_loaded_component_code(&code_cache, &loaded_component.name, &loaded_component.version, &loaded_component.code);
                            }

                            Some(loaded_component)
                        },
                        None => None
                    }
                },
                None => None
            }
        }
    }
}

fn remove_logging_code(code: &String) -> String {
    let new_code = code
        .replace("includes('stage.textalk.se')", "includes()")
        .replace("console.log('Putting obj into cache by path', pth);", "")
        .replace("console.log(\"%c TWAPI CALL \"+e,\"color: #7D4585; font-weight: bold;\",t),", "")
        .replace("console.log(\"%c TWAPI RESULT \"+e,\"color: green; font-weight: bold;\",r.result,t),", "")
        .replace("console.log('%cDEPRECATED: tws-react from1to1x %c import from tws-core instead', 'color: red', 'color: #000');", "")
        .replace("console.log('%cDEPRECATED: tws-react jed %c import from tws-core instead', 'color: red', 'color: #000');", "");

    new_code
}

fn remote_js_paths_collection_into_loaded_component(js_paths_collection: load::JSPathCollection, ds_api_server: &String, disable_logging: bool, verbose: bool) -> Option<LoadedComponent> {
    let mut component_code = String::new();

    for js_path in js_paths_collection.paths.into_iter() {
        let loaded_content = load::get_remote_component_file_contents(
            &ds_api_server,
            &js_paths_collection.component_name,
            &js_paths_collection.component_version,
            &js_path,
        );
        match loaded_content {
            Some(content) => {
                if verbose {
                    println!("{}/{} ({} bytes)", &js_paths_collection.component_name, &js_paths_collection.component_version, &content.len());
                }
                component_code.push_str(&content);
            }
            None => return None,
        }
    }
    
    if disable_logging {
        component_code = remove_logging_code(&component_code);
    }

    Some(LoadedComponent {
        name: js_paths_collection.component_name.clone(),
        version: js_paths_collection.component_version.clone(),
        code: component_code.clone()
    })
}

fn override_exists(override_folder: &String, component_name: &String) -> bool {
    let component_folder = Path::new(override_folder).join(component_name);
    component_folder.exists()
}

fn get_overidden_loaded_component(override_folder: &String, component_name: &String) -> Option<LoadedComponent> {
    let component_path = Path::new(override_folder).join(component_name);
    let diversity_json_path = component_path.join("diversity.json");
    if diversity_json_path.exists() {
        match fs::read_to_string(diversity_json_path) {
            Ok(diversity_json_contents) => {
                let mut component_code = String::new();
                let maybe_component_js_paths = load::get_component_js_paths(&diversity_json_contents);
                if let Some(component_js_paths) = maybe_component_js_paths {
                    for js_path_str in component_js_paths.into_iter() {
                        let js_path = component_path.join(&js_path_str);
                        if js_path.exists() {
                            if let Ok(js_code) = fs::read_to_string(js_path) {
                                component_code.push_str(&js_code);
                            }
                        } else {
                            println!("File specified in diversity json does not exist {}", &js_path.to_str().unwrap_or(&String::new()));
                        }
                    }
                    return Some(LoadedComponent {
                        name: component_name.clone(),
                        version: String::new(),
                        code: component_code.clone(),
                    })
                }
            },
            Err(_) => {
                println!("Could not read diversity json for override {} at {}", component_name, override_folder);
            }
        }
    }
    None
}

fn main() {
    let matches = App::new("concatter")
        .version("1.0")
        .about("Concatenate and serve javascript from diversity components by referencing name and version in query params")
        .author("Anton Gustafsson")
        .arg(Arg::with_name("server")
            .short("s")
            .long("server")
            .help("DS-API host with optional port. Default: antonstage.textalk.se:8383")
            .takes_value(true))
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .help("Port to run server on. Default: 8080")
            .takes_value(true))
        .arg(Arg::with_name("cache")
            .short("c")
            .long("cache")
            .help("Cache code for all loaded components for faster serving. Exclude components with --cache-exclude."))
        .arg(Arg::with_name("files")
            .short("e")
            .long("cache-exclude")
            .help("Exclude component bundle files from cache when cache is active. Usage: --cache-exclude [component1] [component2] [...]")
            .takes_value(true)
            .min_values(0)
            .requires("cache"))
        .arg(Arg::with_name("localfolder")
            .short("l")
            .long("local-folder")
            .help("Load components from a local override folder.")
            .takes_value(true)
            .min_values(0))
        .arg(Arg::with_name("nolog")
            .short("n")
            .long("no-log")
            .help("Remove excessive logging from source code"))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .help("Print which components are loaded from server"))
        .get_matches();

    let ds_api_server = String::from(
        matches
            .value_of("server")
            .unwrap_or("antonstage.textalk.se:8383"),
    );
    let port = String::from(matches.value_of("port").unwrap_or("8080"));
    let use_cache = matches.is_present("cache");
    let uncached_files: Vec<String> = match matches.values_of("files") {
        Some(files) => files.map(|value| String::from(value)).collect(),
        None => vec![],
    };
    let maybe_override_folder = matches.value_of("localfolder").and_then(|value| Some(String::from(value)));
    let disable_logging = matches.is_present("nolog");
    let verbose = matches.is_present("verbose");
    let mut server_address = String::from("127.0.0.1");
    server_address.push_str(":");
    server_address.push_str(&port);
    let server = tiny_http::Server::http(&server_address).unwrap();
    let code_cache = chashmap::CHashMap::new();

    println!("Server listening on {}", server_address);

    loop {
        // blocks until the next request is received
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {
                println!("error: {}", e);
                break;
            }
        };

        let request_url = String::from(request.url());

        match parse_params(&request_url) {
            Some(params) => {
                let components: Vec<Option<LoadedComponent>> = params.par_iter().map(|param| {
                    let component_name = &param.name;
                    let maybe_component_version = &param.value;

                    match &maybe_component_version {
                        Some(component_version) => {
                            let populate_cache = !uncached_files.contains(&component_name) && use_cache;

                            if let Some(override_folder) = &maybe_override_folder {
                                if override_exists(&override_folder, &component_name) {
                                    if verbose {
                                        println!("Serving local override for {}", &component_name);
                                    }
                                    return get_overidden_loaded_component(&override_folder, &component_name)
                                }
                            }
                            get_remote_loaded_component(&ds_api_server, populate_cache, disable_logging, &code_cache, &component_name, &component_version, verbose)
                        }
                        None => None,
                    }
                }).collect();

                let mut js_bundle = String::new();

                for maybe_component in components.into_iter() {
                    match maybe_component {
                        Some(loaded_component) => {
                            push_component_code(&mut js_bundle, &loaded_component);
                        }
                        None => (),
                    }
                }

                let mut response = tiny_http::Response::from_string(js_bundle);
                response.add_header(
                    tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                        .unwrap(),
                );
                match request.respond(response) {
                    Ok(_) => (),
                    Err(_) => (),
                }
            }
            None => match request.respond(tiny_http::Response::from_string("No params")) {
                Ok(_) => (),
                Err(_) => (),
            },
        };
    }
}
