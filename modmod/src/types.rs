use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use crate::{book::Book, error::OutputError, load::Load, to_numbered_tag, Result};
use globset::{Glob, GlobSetBuilder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub path: PathBuf,
    #[serde(default = "exercise_description_md")]
    pub description: PathBuf,
    #[serde(default = "exercise_includes")]
    pub includes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Topic {
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<PathBuf>,
    #[serde(default)]
    pub exercises: Vec<Exercise>,
    #[serde(default)]
    pub summary: Vec<String>,
    #[serde(default)]
    pub objectives: Vec<String>,
    #[serde(default = "topic_slides_md")]
    pub content: PathBuf,
    #[serde(default)]
    pub further_reading: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Unit {
    pub name: String,
    #[serde(default = "unit_template_md")]
    pub template: PathBuf,
    pub topics: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct Module {
    pub units: Vec<Unit>,
    pub name: String,
    pub description: String,
}

impl PathTo<Module> {
    pub fn load_topics(&self) -> Result<Vec<(&Unit, Vec<PathTo<Topic>>)>> {
        let Self { path, data } = self;
        let mut units = Vec::with_capacity(data.units.len());
        let base_path = path.parent().unwrap();

        for unit in data.units.iter() {
            let mut topics = Vec::with_capacity(unit.topics.len());
            for topic in unit.topics.iter() {
                topics.push(Topic::load(topic, Some(base_path))?)
            }
            units.push((unit, topics));
        }

        Ok(units)
    }
}

#[derive(Debug, Deserialize)]
pub struct Track {
    pub name: String,
    pub modules: Vec<PathBuf>,
    pub excluded_topics: Vec<PathBuf>,
}

impl Track {
    pub fn load_excluded_topics(&self) -> Result<Vec<Topic>> {
        todo!()
    }

    pub fn load(path: impl AsRef<Path>) -> Result<PathTo<Self>> {
        Load::load(path.as_ref(), None)
    }

    pub fn render(
        path: impl AsRef<Path>,
        output_dir: impl AsRef<Path>,
        clear_output: bool,
    ) -> Result<()> {
        use std::io::Write;
        let output_dir = output_dir.as_ref();
        if clear_output {
            if output_dir.exists() {
                fs::remove_dir_all(output_dir)?;
            }
            fs::create_dir_all(output_dir)?;
        } else {
            let None = fs::read_dir(output_dir)?.next() else {
                return Err(OutputError::NotEmpty.into());
            };
        }

        let track = Self::load(path)?;

        let mut book_builder = Book::builder("test");

        for (module, i_mod) in track.load_modules()?.iter().zip(1..) {
            let module_tag = to_numbered_tag(&module.data.name, i_mod);
            let module_out_dir = output_dir.join(Path::new(&module_tag));
            fs::create_dir(&module_out_dir)?;
            let mut chapter = book_builder.chapter(&module.data.name);

            for ((unit, topics), i_unit) in module.load_topics()?.iter().zip(1..) {
                let mut section = chapter.section(&unit.name);
                let unit_tag = to_numbered_tag(&unit.name, i_unit);
                let unit_out_dir = module_out_dir.join(unit_tag);
                fs::create_dir(&unit_out_dir)?;
                let exercise_out_dir = unit_out_dir.join("exercises");
                fs::create_dir(&exercise_out_dir)?;

                let template =
                    fs::read_to_string(module.path.parent().unwrap().join(&unit.template))?;

                let mut topic_content = String::new();
                let mut topic_objectives = String::new();
                let mut topic_summary = String::new();
                for topic in topics {
                    let topic_slides =
                        fs::read_to_string(topic.path.parent().unwrap().join(&topic.data.content))?;
                    topic_content += "---\n\n";
                    topic_content += topic_slides.trim();
                    topic_content += "\n";

                    for objective in &topic.data.objectives {
                        topic_objectives += &format!("- {}\n", objective.trim());
                    }
                    for item in &topic.data.summary {
                        topic_summary += &format!("- {}\n", item.trim());
                    }

                    for (exercise, i_exercise) in topic.data.exercises.iter().zip(1..) {
                        let exercise_dir = topic.path.parent().unwrap().join(&exercise.path);
                        section.subsection(&exercise.name, exercise_dir.join(&exercise.description));
                        let content = fs_extra::dir::get_dir_content(&exercise_dir).unwrap();
                        let exercise_tag = to_numbered_tag(&exercise.name, i_exercise);
                        let mut globset = GlobSetBuilder::new();
                        for include in &exercise.includes {
                            globset.add(
                                Glob::new(exercise_dir.join(include).to_str().unwrap()).unwrap(),
                            );
                        }
                        let globset = globset.build().unwrap();
                        for included_file in content.files.iter().filter(|f| globset.is_match(f)) {
                            let file_relative = Path::new(&included_file)
                                .strip_prefix(&exercise_dir)
                                .unwrap();
                            let dest = exercise_out_dir.join(&exercise_tag).join(file_relative);
                            fs::create_dir_all(dest.parent().unwrap())?;
                            fs::copy(included_file, dest)?;
                        }
                    }
                }
                section.add();

                let unit_content = template
                    .replace("#[modmod:content]\n", &topic_content)
                    .replace("#[modmod:objectives]", &topic_objectives)
                    .replace("#[modmod:summary]", &topic_summary);
                let mut unit_slides = File::create(unit_out_dir.join("slides.md"))?;
                write!(unit_slides, "{unit_content}")?;
            }
            chapter.add();
        }

        let book = dbg!(book_builder.build());
        book.render(&output_dir)?;
        Ok(())
    }
}

impl PathTo<Track> {
    pub fn load_modules(&self) -> Result<Vec<PathTo<Module>>> {
        let Self { path, data } = self;
        let mut modules = Vec::with_capacity(data.modules.len());
        let base_path = Some(path.parent().unwrap());

        for path in data.modules.iter() {
            modules.push(Module::load(path, base_path)?);
        }

        Ok(modules)
    }
}

#[derive(Debug)]
pub struct PathTo<T> {
    pub data: T,
    pub path: PathBuf,
}

fn topic_slides_md() -> PathBuf {
    PathBuf::from("slides.md")
}

fn unit_template_md() -> PathBuf {
    PathBuf::from("template.md")
}

fn exercise_description_md() -> PathBuf {
    PathBuf::from("description.md")
}

fn exercise_includes() -> Vec<String> {
    ["Cargo.toml", "Cargo.lock", "src/*/**"]
        .map(String::from)
        .to_vec()
}
