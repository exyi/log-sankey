#![feature(map_first_last)]
#![allow(dead_code)]
#![allow(unused_imports)]
mod parser;
mod streamutil;
mod session_analyzer;
#[macro_use] mod util;
mod stats;

use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};
use stats::{calc_stats, make_inverse_mapping, calc_graph};

use std::{panic, sync::Mutex, collections::{HashMap, HashSet}};

use futures::{StreamExt, stream};
use js_sys::Uint8Array;
use session_analyzer::Session;
use wasm_bindgen::{prelude::*, JsValue, JsCast, JsObject, convert::{IntoWasmAbi, WasmAbi, ResultAbi}, describe::WasmDescribe};
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, window, Response};
use wasm_streams::ReadableStream;

use crate::stats::StatsOptions;

lazy_static! {
    static ref SESSIONS: Mutex<Vec<Session>> = Mutex::new(vec![]);
    static ref SYMBOL_TABLE: Mutex<parser::GlobalTable> = Mutex::new(parser::GlobalTable::new());
}

#[wasm_bindgen]
pub fn clear_sessions() {
    SESSIONS.lock().unwrap().clear();
    let mut t = SYMBOL_TABLE.lock().unwrap();
    *t = parser::GlobalTable::new();
}

#[wasm_bindgen]
pub fn usage_stats_by_path(opt: StatsOptions) -> JsValue {
    let sessions = SESSIONS.lock().unwrap();
    let symbols = SYMBOL_TABLE.lock().unwrap();

    let r = calc_stats(
        &sessions,
        &opt,
        true,
        |s, i| s.actions[i],
        make_inverse_mapping(&symbols.path, &"".to_owned())
    );

    JsValue::from_serde(&r).unwrap()
}

#[wasm_bindgen]
pub fn usage_stats_by_ua(opt: StatsOptions) -> JsValue {
    let sessions = SESSIONS.lock().unwrap();
    let symbols = SYMBOL_TABLE.lock().unwrap();

    let r = calc_stats(
        &sessions,
        &opt,
        false,
        |s, _i| s.user_agent,
        make_inverse_mapping(&symbols.user_agent, &"".to_owned())
    );

    JsValue::from_serde(&r).unwrap()
}

#[wasm_bindgen]
pub fn usage_stats_by_referer(opt: StatsOptions) -> JsValue {
    let sessions = SESSIONS.lock().unwrap();
    let symbols = SYMBOL_TABLE.lock().unwrap();

    let r = calc_stats(
        &sessions,
        &opt,
        false,
        |s, _i| s.user_agent,
        make_inverse_mapping(&symbols.referer, &"".to_owned())
    );

    JsValue::from_serde(&r).unwrap()
}

#[wasm_bindgen]
pub fn usage_transfer_graph(opt: StatsOptions, graph_length: usize, must_contain: &str, must_startwith: &str) -> JsValue {
    let sessions = SESSIONS.lock().unwrap();
    let symbols = SYMBOL_TABLE.lock().unwrap();

    let g = calc_graph(&sessions, &symbols, graph_length, &opt, must_contain, must_startwith);

    JsValue::from_serde(&g).unwrap()
}

#[wasm_bindgen]
pub async fn load_logs(
    input: Vec<wasm_streams::readable::sys::ReadableStream>,
    pattern: String,
    date_pattern: String,
    capture_idxs: Vec<usize>,
    ignore_query_string: bool,
    max_age: u32,
    report_progress: js_sys::Function
) -> u32 {
    panic::set_hook(Box::new(console_error_panic_hook::hook));


    let parser = parser::create_parser(&pattern, capture_idxs, &date_pattern, ignore_query_string);
    let str: Vec<_> = input.into_iter().map(|i| ReadableStream::from_raw(i)).collect();

    let input_streams = str.into_iter().map(|x| {
        let byte_stream = x.into_stream().map(|jsvalue| Uint8Array::from(jsvalue.unwrap()).to_vec());
        let byte_stream = byte_stream.map(|x| {
            _ = report_progress.call1(&JsValue::null(), &JsValue::from_f64(x.len() as f64));
            x
        });
        streamutil::bytes_to_lines(byte_stream)
    });

    let line_stream = stream::iter(input_streams).flatten();

    let mut symbol_table = SYMBOL_TABLE.lock().unwrap();

    let loglines: Vec<_> = line_stream.map(|lines| lines.iter().filter_map(
        |line| {

            if line.contains("Bot/") || line.contains("bot/") {
                // optimization: skip lines that are just fucking bots
                return None
            }
            
            match parser::parse_line_handwritten1(&parser, &mut symbol_table, line) {
                Ok(l) => Some(l),
                Err(e) => {
                    // console::error_1(&JsValue::from_str(&e.to_string()));
                    log!("Could not parse {}: {}", line, e);
                    None
                }
        }}).collect::<Vec<_>>()).collect::<Vec<_>>().await;
    let mut sessions: Vec<Session> = session_analyzer::get_sessions(&symbol_table, stream::iter(loglines), max_age).collect().await;
    log!("Sessions (unfiltered): {}", sessions.len());
    let bots = symbol_table.get_bots();
    log!("Bots: {:?}", bots.iter().map(|&(_, b)| b).collect::<Vec<&str>>());
    let bots: HashSet<u32> = bots.iter().map(|&(c, _)| c).collect();
    sessions.retain(|s| !bots.contains(&s.user_agent));
    log!("Sessions (filtered): {}", sessions.len());
    log!("Session actions: {}", sessions.iter().map(|s| s.actions.len()).sum::<usize>());
    SESSIONS.lock().unwrap().append(&mut sessions);

    0
    // loglines.count().await as u32
}

