use minijinja::Environment;
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
        env.set_loader(|name| -> Result<Option<String>, minijinja::Error> {
            let path = std::path::Path::new("templates").join(name);
            match std::fs::read_to_string(path) {
                Ok(content) => Ok(Some(content)),
                Err(_) => Ok(None),
            }
        });

        Self {
            env: RwLock::new(env),
        }
    }

    /// Render a template with the given context
    pub fn render(
        &self,
        template_name: &str,
        context: &serde_json::Value,
    ) -> Result<String, String> {
        let env = self.env.read().unwrap();

        // get the template
        let template = env
            .get_template(template_name)
            .map_err(|e| format!("Failed to load template '{}': {}", template_name, e))?;

        // render the template and return the result
        template
            .render(context)
            .map_err(|e| format!("Failed to render template '{}': {}", template_name, e))
    }

    /// Reload the templates
    pub fn reload(&self) -> Result<(), String> {
        let mut env = self.env.write().unwrap();

        //reload templates
        env.set_loader(|name| -> Result<Option<String>, minijinja::Error> {
            let path = std::path::Path::new("templates").join(name);
            match std::fs::read_to_string(path) {
                Ok(content) => Ok(Some(content)),
                Err(_) => Ok(None),
            }
        });

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
