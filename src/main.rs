extern crate clap; 
extern crate rayon;
extern crate reqwest;
use tiny_http;
use rayon::prelude::*;
use clap::{Arg, App};
mod load;

#[derive(Debug)]
struct QueryParam {
    name: String,
    value: Option<String>,
}

struct LoadedComponent {
    name: String,
    version: String,
    code: String
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

fn cache_loaded_component_code(code_cache: &chashmap::CHashMap<String, String>, component_name: &String, component_version: &String, component_code: &String) {
    let cache_key = build_component_cache_key(&component_name, &component_version);
    code_cache.insert_new(cache_key, component_code.clone());
}

fn get_cached_component_code(code_cache: &chashmap::CHashMap<String, String>, component_name: &String, component_version: &String) -> Option<String> {
    let cache_key = build_component_cache_key(&component_name, &component_version);
    match code_cache.get(&cache_key) {
        Some(cache_value_read_guard) => {
            Some(String::from(cache_value_read_guard.as_str()))
        },
        None => None
    }
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
        .arg(Arg::with_name("files")
            .short("e")
            .long("cache-exclude")
            .help("Exclude component bundle files from being cached in memory.")
            .takes_value(true)
            .min_values(0))
        .arg(Arg::with_name("nolog")
            .short("n")
            .long("no-log")
            .help("Remove excessive logging from source code"))
        .get_matches();

    let ds_api_server = String::from(matches.value_of("server").unwrap_or("antonstage.textalk.se:8383"));
    let port = String::from(matches.value_of("port").unwrap_or("8080"));
    let uncached_files: Vec<String> = match matches.values_of("files") {
        Some(files) => files.map(|value| String::from(value)).collect(),
        None => vec![]
    };
    let disable_logging = matches.is_present("nolog");
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
                            let mut component_code = String::new();

                            match get_cached_component_code(&code_cache, &component_name, &component_version) {
                                Some(cached_component_code) => {
                                    component_code.push_str(&cached_component_code);
                                },
                                None => {
                                    let js_paths =
                                        load::get_component_js_paths(&ds_api_server, &component_name, &component_version);

                                    for js_path in js_paths.into_iter() {
                                        let loaded_content = load::get_component_file_contents(
                                            &ds_api_server,
                                            &component_name,
                                            &component_version,
                                            &js_path,
                                        );
                                        match loaded_content {
                                            Some(content) => {
                                                println!("{}/{} ({} bytes)", &component_name, &component_version, &content.len());
                                                component_code.push_str(&content);
                                            }
                                            None => (),
                                        }
                                    }

                                    if !uncached_files.contains(&component_name) {
                                        if disable_logging {
                                            component_code = component_code
                                            .replace("includes('stage.textalk.se')", "includes()")
                                            .replace("console.log('Putting obj into cache by path', pth);", "")
                                            .replace("console.log(\"%c TWAPI CALL \"+e,\"color: #7D4585; font-weight: bold;\",t),", "")
                                            .replace("console.log(\"%c TWAPI RESULT \"+e,\"color: green; font-weight: bold;\",r.result,t),", "")
                                            .replace("console.log('%cDEPRECATED: tws-react from1to1x %c import from tws-core instead', 'color: red', 'color: #000');", "")
                                            .replace("console.log('%cDEPRECATED: tws-react jed %c import from tws-core instead', 'color: red', 'color: #000');", "");
                                        }
                                        cache_loaded_component_code(&code_cache, &component_name, &component_version, &component_code);
                                    }
                                }
                            }

                            let loaded_component =
                            LoadedComponent {
                                name: component_name.clone(),
                                version: component_version.clone(),
                                code: component_code.clone(),
                            };
                            Some(loaded_component)
                        }
                        None => None,
                    }
                }).collect();
                
                let mut js_bundle = String::new();

                for maybe_component in components.into_iter() {
                    match maybe_component {
                        Some(loaded_component) => {
                            push_component_code(&mut js_bundle, &loaded_component);
                        },
                        None => ()
                    }
                }

                let mut response = tiny_http::Response::from_string(js_bundle);
                response.add_header(
                    tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                        .unwrap(),
                );
                match request.respond(response) {
                    Ok(_) => (),
                    Err(_) => ()
                }
            }
            None => {
                match request.respond(tiny_http::Response::from_string("No params")) {
                    Ok(_) => (),
                    Err(_) => ()
                }
            }
        };
    }
}
