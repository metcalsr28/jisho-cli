use crate::Value;

#[derive(Default, Debug, Clone)]
pub struct Options {
    pub limit: usize,
    pub query: String,
    pub interactive: bool,
}

//
// --- Value helper
//

pub fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        _ => unreachable!(),
    }
}

pub fn value_to_str(value: &Value) -> &str {
    match value {
        Value::String(s) => s,
        _ => unreachable!(),
    }
}

pub fn value_to_arr(value: &Value) -> &Vec<Value> {
    match value {
        Value::Array(a) => a,
        _ => unreachable!(),
    }
}
