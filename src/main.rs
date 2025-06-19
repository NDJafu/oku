use std::fs;
use std::io::Write;

use clap::Parser;
use convert_case::{Case, Casing};
use dialoguer::{Confirm, Input, Select};
use handlebars::{handlebars_helper, Handlebars};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  #[arg(help = "Runs a specific generator directly")]
  generator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Prompt {
  name: String,
  message: String,
  #[serde(default)]
  r#type: String,
  choices: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Action {
  #[serde(default)]
  r#type: String,
  template: String,
  output: String,
}

#[derive(Debug, Deserialize)]
struct Meta {
  prompts: Vec<Prompt>,
  actions: Vec<Action>,
}

fn load_generators() -> Vec<String> {
  let cwd = std::env::current_dir().unwrap();
  fs::read_dir(&cwd.join("generators"))
    .expect("Failed to find generators folder!")
    .filter_map(Result::ok)
    .filter(|e| e.path().is_dir())
    .map(|e| e.file_name().to_string_lossy().to_string())
    .collect()
}

fn main() {
  let args = Args::parse();
  let cwd = std::env::current_dir().unwrap();
  let dirs = load_generators();

  let selection = match args.generator {
    Some(g) => g,
    None => {
      let result = Select::new()
        .with_prompt("Choose a generator")
        .items(&dirs)
        .default(0)
        .interact()
        .unwrap();
      dirs[result].clone()
    }
  };

  let selected_gen = cwd.join("generators").join(selection);
  let yaml = fs::read_to_string(selected_gen.join("meta.yaml")).expect("Failed to read meta.yaml");
  let meta: Meta = serde_yaml::from_str(&yaml).expect("Invalid meta file");

  let mut answers = Map::new();
  for prompt in meta.prompts {
    let key = prompt.name;
    let message = prompt.message;
    let input_type = prompt.r#type.as_str();
    let answer = match input_type {
      "confirm" => {
        let val = Confirm::new().with_prompt(&message).interact().unwrap();
        Value::Bool(val)
      }
      "list" => {
        let choices = prompt
          .choices
          .expect(&format!("There's no choices for {}", key));

        let val = Select::new()
          .with_prompt(&message)
          .items(&choices)
          .default(0)
          .interact()
          .unwrap();

        Value::String(choices[val].clone())
      }
      _ => {
        let val: String = Input::new().with_prompt(&message).interact_text().unwrap();
        Value::String(val)
      }
    };
    answers.insert(key, answer);
  }

  handlebars_helper!(pascal_case: |x: str| x.to_case(Case::Pascal));
  handlebars_helper!(camel_case: |x: str| x.to_case(Case::Camel));
  handlebars_helper!(kebab_case: |x: str| x.to_case(Case::Kebab));
  handlebars_helper!(snake_case: |x: str| x.to_case(Case::Snake));

  let mut handlebars = Handlebars::new();
  let context = Value::Object(answers);

  handlebars.register_helper("pascalCase", Box::new(pascal_case));
  handlebars.register_helper("camelCase", Box::new(camel_case));
  handlebars.register_helper("kebabCase", Box::new(kebab_case));
  handlebars.register_helper("snakeCase", Box::new(snake_case));

  for action in meta.actions {
    let action_type = action.r#type.as_str();
    let template = action.template;
    let output = action.output;

    let template_str = fs::read_to_string(selected_gen.join("templates").join(&template))
      .expect(&format!("Can't find template: {}", template));

    let rendered = handlebars.render_template(&template_str, &context).unwrap();
    let output_path = handlebars.render_template(&output, &context).unwrap();

    match action_type {
      _ => {
        let output_str = cwd.join(output_path);

        if let Some(parent) = output_str.parent() {
          fs::create_dir_all(parent).expect("Can't create directories");
        }

        let mut file = fs::File::create(&output_str).expect("Can't create file");
        writeln!(file, "{}", rendered).unwrap();
      }
    }
  }
}
