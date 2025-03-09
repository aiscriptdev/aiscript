use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct ProjectGenerator {
    project_name: String,
    project_path: PathBuf,
}

impl ProjectGenerator {
    pub fn new(project_name: &str) -> Self {
        let project_path = PathBuf::from(project_name);
        Self {
            project_name: project_name.to_string(),
            project_path,
        }
    }

    pub fn generate(&self) -> Result<(), String> {
        // Check if directory already exists
        if self.project_path.exists() {
            return Err(format!(
                "Error: Directory '{}' already exists",
                self.project_name
            ));
        }

        // Create project directory
        fs::create_dir_all(&self.project_path).map_err(|e| {
            format!(
                "Failed to create project directory '{}': {}",
                self.project_name, e
            )
        })?;

        // Create standard directories
        self.create_directories()?;

        // Create project.toml
        self.create_project_toml()?;

        // Create basic example file
        self.create_example_file()?;

        println!(
            "Successfully created new AIScript project: {}",
            self.project_name
        );
        println!("Project structure:");
        println!("{}", self.display_project_structure());
        println!();
        println!("Run `aiscript serve` to start the server.");

        Ok(())
    }

    fn create_directories(&self) -> Result<(), String> {
        let dirs = vec!["lib", "routes"];

        for dir in dirs {
            let dir_path = self.project_path.join(dir);
            fs::create_dir_all(&dir_path).map_err(|e| {
                format!("Failed to create directory '{}': {}", dir_path.display(), e)
            })?;
        }

        Ok(())
    }

    fn create_project_toml(&self) -> Result<(), String> {
        let toml_path = self.project_path.join("project.toml");
        let username = whoami::username();

        let toml_content = format!(
            r#"[project]
name = "{}"
description = "An AIScript project"
version = "0.1.0"
authors = ["{}"]

[network]
host = "0.0.0.0"
port = 8000

[apidoc]
enabled = true
type = "redoc"
path = "/docs"
"#,
            self.project_name, username
        );

        let mut file = fs::File::create(&toml_path)
            .map_err(|e| format!("Failed to create project.toml: {}", e))?;

        file.write_all(toml_content.as_bytes())
            .map_err(|e| format!("Failed to write to project.toml: {}", e))?;

        Ok(())
    }

    fn create_example_file(&self) -> Result<(), String> {
        let routes_dir = self.project_path.join("routes");
        let example_path = routes_dir.join("index.ai");

        let example_content = r#"// Example AIScript route handler
get /hello {
    query {
        name: str
    }

    return { message: f"Hello, {query.name}!" };
}
"#;

        let mut file = fs::File::create(&example_path)
            .map_err(|e| format!("Failed to create example file: {}", e))?;

        file.write_all(example_content.as_bytes())
            .map_err(|e| format!("Failed to write to example file: {}", e))?;

        Ok(())
    }

    fn display_project_structure(&self) -> String {
        let mut result = format!("{}\n", self.project_name);
        result.push_str("├── lib/\n");
        result.push_str("├── routes/\n");
        result.push_str("│   └── index.ai\n");
        result.push_str("└── project.toml\n");

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_project_generator() {
        // Use tempdir to ensure test files are cleaned up
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create a test project in the temp directory
        let project_name = "test_project";

        // Create an absolute path for the project
        let project_path = temp_path.join(project_name);

        // Create a generator with the project name
        let generator = ProjectGenerator::new(project_name);

        // Override the project path for testing
        let generator = ProjectGenerator {
            project_name: project_name.to_string(),
            project_path: project_path.clone(),
        };

        let result = generator.generate();

        assert!(result.is_ok(), "Project generation failed: {:?}", result);

        // Verify project structure
        assert!(project_path.exists(), "Project directory not created");
        assert!(
            project_path.join("lib").exists(),
            "lib directory not created"
        );
        assert!(
            project_path.join("routes").exists(),
            "routes directory not created"
        );
        assert!(
            project_path.join("project.toml").exists(),
            "project.toml not created"
        );
        assert!(
            project_path.join("routes/index.ai").exists(),
            "Example file not created"
        );

        // Verify project.toml content
        let toml_content = fs::read_to_string(project_path.join("project.toml")).unwrap();
        assert!(toml_content.contains(&format!("name = \"{}\"", project_name)));
        assert!(toml_content.contains("version = \"0.1.0\""));

        // Verify example file content
        let example_content = fs::read_to_string(project_path.join("routes/index.ai")).unwrap();
        assert!(example_content.contains("get /hello"));
    }

    #[test]
    fn test_project_already_exists() {
        // Use tempdir to ensure test files are cleaned up
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create a directory that will conflict
        let project_name = "existing_project";
        let project_path = temp_path.join(project_name);
        fs::create_dir_all(&project_path).unwrap();

        // Create a generator with absolute path
        let generator = ProjectGenerator {
            project_name: project_name.to_string(),
            project_path,
        };

        let result = generator.generate();

        assert!(
            result.is_err(),
            "Project generation should fail for existing directory"
        );
        if let Err(err) = result {
            assert!(
                err.contains("already exists"),
                "Wrong error message: {}",
                err
            );
        }
    }
}
