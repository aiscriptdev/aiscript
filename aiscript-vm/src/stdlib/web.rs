use crate::value::Value; 
use aiscript_runtime::template::get_template_engine; 

/// Render a template with the given context  
pub fn render(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("render expects 2 arguments: template name and context".to_string());
    }

    //get the template name 
    let template_name = args[0].as_str().ok_or_else(|| "First argument must be a string (template name)".to_string())?;

    //get the context 
    let context = serde_json::to_value(&args[1])
        .map_err(|e| format!("Failed to convert context to JSON: {}", e))?;

    //render the template  
    let result = get_template_engine().render(template_name, context)?;

    OK(Value::String(result.into()))
    
}