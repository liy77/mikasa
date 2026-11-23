//! Transpile checks for Mikasa language features (wrappers, keypaths, actors, …).

use mikasa_syntax::tokenizer::tokenizer::Tokenizer;

fn transpile(src: &str) -> String {
    let tokens = Tokenizer::new(src.to_string()).tokenize();
    mikasa_parser::parser::parse(tokens)
}

#[test]
fn codable_struct_emits_serde_derive() {
    let rust = transpile("codable struct User { name: string  age: int }\n");
    assert!(
        rust.contains("Serialize") && rust.contains("Deserialize"),
        "missing serde derive: {rust}"
    );
    assert!(rust.contains("struct User"), "got: {rust}");
}

#[test]
fn codable_emits_encoder_and_to_json() {
    let rust = transpile(
        "codable struct User { name: string  age: int }\n\
         u = User { name: \"a\".to_string(), age: 1 }\n\
         js = u.to_json()\n\
         back = decode(User, js)\n",
    );
    assert!(rust.contains("struct JSONEncoder"), "encoder missing: {rust}");
    assert!(rust.contains("struct JSONDecoder"), "decoder missing: {rust}");
    assert!(rust.contains("fn to_json(&self)"), "to_json missing: {rust}");
    assert!(
        rust.contains("fn from_json(s: &str)"),
        "from_json missing: {rust}"
    );
    assert!(
        rust.contains("JSONDecoder::new().decode::<User>"),
        "decode sugar missing: {rust}"
    );
}

#[test]
fn keypath_emits_helper() {
    let rust = transpile(
        "struct Person { name: string }\n\
         kp = \\Person.name\n",
    );
    assert!(
        rust.contains("__MikaKeyPath"),
        "missing keypath helper: {rust}"
    );
    assert!(rust.contains("&__r.name"), "got: {rust}");
}

#[test]
fn actor_emits_mutex_wrapper() {
    let rust = transpile(
        "actor Counter {\n\
           count: int\n\
           func void inc(self) { self.count += 1 }\n\
           func int get(self) { return self.count }\n\
         }\n",
    );
    assert!(rust.contains("Mutex"), "got: {rust}");
    assert!(rust.contains("CounterInner"), "got: {rust}");
    assert!(rust.contains("fn inc(&self"), "got: {rust}");
}

#[test]
fn result_builder_rewrites_body() {
    let rust = transpile(
        "@resultBuilder\n\
         struct IntList {}\n\
         impl IntList {\n\
           func Vec<int> buildBlock(parts: Vec<int>) { return parts }\n\
         }\n\
         @IntList\n\
         func Vec<int> demo() {\n\
           1\n\
           2\n\
         }\n",
    );
    assert!(
        rust.contains("IntList::buildBlock"),
        "builder call missing: {rust}"
    );
}

#[test]
fn property_wrapper_rewrites_field_type() {
    let rust = transpile(
        "@propertyWrapper\n\
         struct Clamped { wrappedValue: int }\n\
         struct Form { @Clamped age: int }\n",
    );
    assert!(
        rust.contains("age: Clamped") || rust.contains("age:Clamped"),
        "wrapper field type missing: {rust}"
    );
    assert!(
        !rust.contains("age: i64") && !rust.contains("age: int"),
        "original type leaked: {rust}"
    );
}

#[test]
fn observers_emit_getter_setter() {
    let rust = transpile(
        "class Thermometer {\n\
           temp: f64 {\n\
             willSet { println!(\"{}\", newValue) }\n\
             didSet { println!(\"{}\", oldValue) }\n\
           }\n\
         }\n",
    );
    assert!(rust.contains("__temp"), "storage missing: {rust}");
    assert!(rust.contains("fn temp(&self)"), "getter missing: {rust}");
    assert!(
        rust.contains("fn set_temp(&mut self"),
        "setter missing: {rust}"
    );
}
