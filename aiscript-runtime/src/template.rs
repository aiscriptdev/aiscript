use minijinja::{Environment, Source};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;


/// Template engine for AIScript
pub struct TemplateEngine {
    env: RwLock<Environment<'static>>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        let mut env = Environment::new();

        //Set the source to the templates directory
        let source = Source::from_path("templates");
        env.set_source(source); 

        Self {
            env: RwLock::new(env),
        }
    }

    /// Render a template with the given context
    pub fn render(&self, template_name: &str, context: serde_json::Value) -> Result<String, String> {
        let env = self.env.read().unwrap();

        //get template 
        let template = env.get_template(template_name).map_err(|e| format!("Failed to get template: {}", e))?;

        //render template
        let rendered = template.render(&context).map_err(|e| format!("Failed to render template: {}", e))?;
    }

    /// Reload the templates
    pub fn reload(&self) -> Result<(), String> {
        let  mut env = self.env.write().unwrap();

        //reload templates 
        let source = Source::from_path("templates");
        env.set_source(source);

        Ok(())
    }
}

//Create a global instance of the template engine 
lazy_static::lazy_static! {
    static ref TEMPLATE_ENGINE: TemplateEngine = TemplateEngine::new();
}

//Get the template engine instance  
pub fn get_template_engine() -> &'static TemplateEngine {
    &TEMPLATE_ENGINE
}