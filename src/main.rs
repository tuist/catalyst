use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "catalyst")]
#[command(about = "Convert Tuist projects to Bazel builds", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Build the project with Bazel
    Build {
        #[arg(
            short,
            long,
            help = "Project directory (defaults to current directory)"
        )]
        path: Option<PathBuf>,
    },
    /// Build and run the app in iOS Simulator
    Run {
        #[arg(
            short,
            long,
            help = "Project directory (defaults to current directory)"
        )]
        path: Option<PathBuf>,

        #[arg(
            short,
            long,
            default_value = "iPhone 16",
            help = "Simulator device to use"
        )]
        simulator: String,

        #[arg(short, long, help = "Target to run (defaults to first app target)")]
        target: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct TuistGraph {
    name: String,
    path: String,
    projects: serde_json::Value, // Array with path string followed by project object
}

#[derive(Debug, Deserialize, Serialize)]
struct TuistProject {
    name: String,
    path: String,
    targets: HashMap<String, TuistTarget>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TuistTarget {
    name: String,
    product: String,
    #[serde(rename = "bundleId")]
    bundle_id: String,
    #[serde(rename = "buildableFolders")]
    buildable_folders: Vec<BuildableFolder>,
    dependencies: Vec<TuistDependency>,
}

#[derive(Debug, Deserialize, Serialize)]
struct BuildableFolder {
    path: String,
    #[serde(rename = "resolvedFiles")]
    resolved_files: Vec<ResolvedFile>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResolvedFile {
    path: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TuistDependency {
    target: Option<TargetReference>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TargetReference {
    name: String,
    #[serde(default)]
    status: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Build { path }) => {
            let project_dir = path.unwrap_or_else(|| PathBuf::from("."));
            build_project(&project_dir)?;
        }
        Some(Commands::Run {
            path,
            simulator,
            target,
        }) => {
            let project_dir = path.unwrap_or_else(|| PathBuf::from("."));
            build_project(&project_dir)?;

            // Get target info from the graph
            let graph = run_tuist_graph(&project_dir)?;
            let (target_name, bundle_id) = find_app_target(&graph, target.as_deref())?;

            run_in_simulator(&project_dir, &target_name, &bundle_id, &simulator)?;
        }
        None => {
            // Default behavior: build
            let project_dir = PathBuf::from(".");
            build_project(&project_dir)?;
        }
    }

    Ok(())
}

fn build_project(project_dir: &Path) -> Result<()> {
    println!("Running catalyst on project: {}", project_dir.display());

    // Step 1: Run tuist graph
    let graph = run_tuist_graph(project_dir)?;

    // Step 2: Get XDG-compliant cache directory
    let cache_dir = get_catalyst_cache_dir()?;
    fs::create_dir_all(&cache_dir)?;

    println!("Using catalyst cache directory: {}", cache_dir.display());

    // Step 3: Generate Bazel files
    generate_bazel_files(&graph, project_dir, &cache_dir)?;

    // Step 4: Run Bazel build
    run_bazel_build(project_dir)?;

    println!("Build completed successfully!");

    Ok(())
}

fn run_tuist_graph(project_dir: &Path) -> Result<TuistGraph> {
    // Create a temporary directory for the graph output
    let temp_dir = std::env::temp_dir();
    let output_dir = temp_dir.join(format!("tuist-graph-{}", std::process::id()));
    fs::create_dir_all(&output_dir)?;

    let graph_file = output_dir.join("graph.json");

    // Ensure cleanup on exit
    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _guard = TempDirGuard(output_dir.clone());

    println!(
        "Running: tuist graph --format json --no-open --output-path {}",
        output_dir.display()
    );

    let status = Command::new("tuist")
        .args([
            "graph",
            "--format",
            "json",
            "--no-open",
            "--output-path",
            output_dir.to_str().unwrap(),
        ])
        .current_dir(project_dir)
        .status()
        .context("Failed to execute tuist graph")?;

    if !status.success() {
        anyhow::bail!("tuist graph command failed");
    }

    // Read the graph file
    let graph_content = fs::read_to_string(&graph_file).context("Failed to read graph file")?;

    let graph: TuistGraph =
        serde_json::from_str(&graph_content).context("Failed to parse tuist graph JSON output")?;

    println!(
        "Successfully parsed Tuist graph for project: {}",
        graph.name
    );

    Ok(graph)
}

fn get_catalyst_cache_dir() -> Result<PathBuf> {
    let cache_base = dirs::cache_dir().context("Failed to determine cache directory")?;

    Ok(cache_base.join("catalyst"))
}

fn generate_bazel_files(graph: &TuistGraph, project_dir: &Path, cache_dir: &Path) -> Result<()> {
    println!("Generating Bazel files...");

    // Generate WORKSPACE file
    generate_workspace_file(project_dir)?;

    // Generate .bazelrc file
    generate_bazelrc(project_dir)?;

    // Parse projects array - it's [path_string, project_object]
    if let Some(projects_array) = graph.projects.as_array() {
        for item in projects_array {
            // Skip string entries (paths), only process objects (projects)
            if item.as_object().is_some() {
                let project: TuistProject = serde_json::from_value(item.clone())
                    .context("Failed to parse project from graph")?;

                println!("Generating BUILD file for project: {}", project.name);
                generate_build_file(&project, project_dir)?;
            }
        }
    }

    // Save graph metadata to cache
    let graph_cache_path = cache_dir.join("graph.json");
    let graph_json = serde_json::to_string_pretty(graph)?;
    fs::write(&graph_cache_path, graph_json).context("Failed to write graph cache")?;

    println!("Saved graph metadata to: {}", graph_cache_path.display());

    Ok(())
}

fn generate_workspace_file(project_dir: &Path) -> Result<()> {
    let workspace_path = project_dir.join("WORKSPACE");

    let workspace_content = r#"workspace(name = "catalyst_workspace")

# Load Apple rules for building iOS/macOS apps
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

http_archive(
    name = "build_bazel_rules_apple",
    sha256 = "b4df908ec14868369021182ab191dbd1f40830c9b300650d5dc389e0b9266c8d",
    url = "https://github.com/bazelbuild/rules_apple/releases/download/3.5.1/rules_apple.3.5.1.tar.gz",
)

load(
    "@build_bazel_rules_apple//apple:repositories.bzl",
    "apple_rules_dependencies",
)

apple_rules_dependencies()

load(
    "@build_bazel_rules_swift//swift:repositories.bzl",
    "swift_rules_dependencies",
)

swift_rules_dependencies()

load(
    "@build_bazel_rules_swift//swift:extras.bzl",
    "swift_rules_extra_dependencies",
)

swift_rules_extra_dependencies()

load(
    "@build_bazel_apple_support//lib:repositories.bzl",
    "apple_support_dependencies",
)

apple_support_dependencies()

# Optional: rules_xcodeproj for generating Xcode projects from Bazel targets
# Uncomment the following to enable Xcode project generation:
#
# http_archive(
#     name = "com_github_buildbuddy_io_rules_xcodeproj",
#     sha256 = "CHECK LATEST RELEASE",
#     url = "https://github.com/MobileNativeFoundation/rules_xcodeproj/releases/download/VERSION/release.tar.gz",
# )
#
# load(
#     "@com_github_buildbuddy_io_rules_xcodeproj//xcodeproj:repositories.bzl",
#     "xcodeproj_rules_dependencies",
# )
#
# xcodeproj_rules_dependencies()
#
# Then add to your BUILD file:
# load("@com_github_buildbuddy_io_rules_xcodeproj//xcodeproj:defs.bzl", "xcodeproj")
#
# xcodeproj(
#     name = "xcodeproj",
#     project_name = "Fixture",
#     targets = [":fixture"],
# )
"#;

    std::fs::write(&workspace_path, workspace_content).context("Failed to write WORKSPACE file")?;

    println!("Generated: {}", workspace_path.display());

    Ok(())
}

fn generate_bazelrc(project_dir: &Path) -> Result<()> {
    let bazelrc_path = project_dir.join(".bazelrc");

    let bazelrc_content = r#"# Build settings
build --apple_platform_type=ios
build --ios_minimum_os=15.0

# Use Xcode toolchain
build --apple_crosstool_top=@local_config_apple_cc//:toolchain
build --crosstool_top=@local_config_apple_cc//:toolchain
build --host_crosstool_top=@local_config_apple_cc//:toolchain

# Output settings
build --verbose_failures
build --announce_rc
"#;

    std::fs::write(&bazelrc_path, bazelrc_content).context("Failed to write .bazelrc file")?;

    println!("Generated: {}", bazelrc_path.display());

    Ok(())
}

fn generate_build_file(project: &TuistProject, project_dir: &Path) -> Result<()> {
    let mut build_content = String::new();

    build_content.push_str("load(\"@build_bazel_rules_apple//apple:ios.bzl\", \"ios_application\", \"ios_unit_test\")\n");
    build_content
        .push_str("load(\"@build_bazel_rules_swift//swift:swift.bzl\", \"swift_library\")\n\n");

    for target in project.targets.values() {
        let target_name_lower = target.name.to_lowercase();

        // Extract Swift source files from buildableFolders
        let mut source_files: Vec<String> = Vec::new();
        let mut resource_files: Vec<String> = Vec::new();

        for folder in &target.buildable_folders {
            for file in &folder.resolved_files {
                let file_path = Path::new(&file.path);
                if let Some(ext) = file_path.extension() {
                    let ext_str = ext.to_string_lossy();
                    if ext_str == "swift" {
                        // Make path relative to project directory
                        if let Ok(rel_path) = Path::new(&file.path).strip_prefix(&project.path) {
                            source_files.push(format!("\"{}\"", rel_path.display()));
                        }
                    } else if ext_str == "xcassets" || ext_str == "storyboard" || ext_str == "xib" {
                        if let Ok(rel_path) = Path::new(&file.path).strip_prefix(&project.path) {
                            resource_files.push(format!("\"{}\"", rel_path.display()));
                        }
                    }
                }
            }
        }

        // Get dependencies
        let deps: Vec<String> = target
            .dependencies
            .iter()
            .filter_map(|dep| {
                dep.target
                    .as_ref()
                    .map(|t| format!("\":{}\"", t.name.to_lowercase()))
            })
            .collect();

        match target.product.as_str() {
            "app" => {
                // Generate swift_library for the app sources
                build_content.push_str(&format!(
                    "swift_library(\n    name = \"{}_lib\",\n",
                    target_name_lower
                ));

                if !source_files.is_empty() {
                    build_content.push_str(&format!(
                        "    srcs = [\n        {},\n    ],\n",
                        source_files.join(",\n        ")
                    ));
                } else {
                    build_content.push_str("    srcs = glob([\"Fixture/Sources/**/*.swift\"]),\n");
                }

                build_content.push_str(&format!("    module_name = \"{}\",\n", target.name));

                if !deps.is_empty() {
                    build_content.push_str(&format!("    deps = [{}],\n", deps.join(", ")));
                }

                build_content.push_str("    visibility = [\"//visibility:public\"],\n)\n\n");

                // Generate minimal Info.plist for the app
                let infoplist_path = project_dir.join(format!("{}-Info.plist", target.name));
                let infoplist_content = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleIdentifier</key>
    <string>{}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSRequiresIPhoneOS</key>
    <true/>
    <key>UILaunchScreen</key>
    <dict/>
</dict>
</plist>
"#,
                    target.name, target.bundle_id, target.name
                );

                fs::write(&infoplist_path, infoplist_content)
                    .context("Failed to write Info.plist")?;

                // Generate ios_application
                build_content.push_str(&format!(
                    "ios_application(\n    name = \"{}\",\n    bundle_id = \"{}\",\n",
                    target_name_lower, target.bundle_id
                ));
                build_content.push_str("    families = [\"iphone\", \"ipad\"],\n");
                build_content.push_str(&format!(
                    "    infoplists = [\"{}-Info.plist\"],\n",
                    target.name
                ));
                build_content.push_str("    minimum_os_version = \"15.0\",\n");
                build_content.push_str(&format!("    deps = [\":{}_lib\"],\n", target_name_lower));
                build_content.push_str(")\n\n");
            }
            "unit_tests" => {
                // Generate test target
                build_content.push_str(&format!(
                    "swift_library(\n    name = \"{}_lib\",\n",
                    target_name_lower
                ));

                if !source_files.is_empty() {
                    build_content.push_str(&format!(
                        "    srcs = [\n        {},\n    ],\n",
                        source_files.join(",\n        ")
                    ));
                } else {
                    build_content.push_str("    srcs = glob([\"Fixture/Tests/**/*.swift\"]),\n");
                }

                build_content.push_str(&format!("    module_name = \"{}\",\n", target.name));
                build_content.push_str("    testonly = True,\n");

                if !deps.is_empty() {
                    build_content.push_str(&format!("    deps = [{}],\n", deps.join(", ")));
                }

                build_content.push_str("    visibility = [\"//visibility:public\"],\n)\n\n");

                // Generate ios_unit_test
                let test_host = target.name.replace("Tests", "").to_lowercase();
                build_content.push_str(&format!(
                    "ios_unit_test(\n    name = \"{}\",\n    bundle_id = \"{}\",\n",
                    target_name_lower, target.bundle_id
                ));
                build_content.push_str("    minimum_os_version = \"15.0\",\n");
                build_content.push_str(&format!("    test_host = \":{}\",\n", test_host));
                build_content.push_str(&format!("    deps = [\":{}_lib\"],\n", target_name_lower));
                build_content.push_str(")\n\n");
            }
            _ => {
                // Default to library
                build_content.push_str(&format!(
                    "swift_library(\n    name = \"{}\",\n",
                    target_name_lower
                ));

                if !source_files.is_empty() {
                    build_content.push_str(&format!(
                        "    srcs = [\n        {},\n    ],\n",
                        source_files.join(",\n        ")
                    ));
                } else {
                    build_content.push_str("    srcs = glob([\"Sources/**/*.swift\"]),\n");
                }

                build_content.push_str(&format!("    module_name = \"{}\",\n", target.name));

                if !deps.is_empty() {
                    build_content.push_str(&format!("    deps = [{}],\n", deps.join(", ")));
                }

                build_content.push_str("    visibility = [\"//visibility:public\"],\n)\n\n");
            }
        }
    }

    let build_path = project_dir.join("BUILD");
    fs::write(&build_path, build_content).context("Failed to write BUILD file")?;

    println!("Generated: {}", build_path.display());

    Ok(())
}

fn run_bazel_build(project_dir: &Path) -> Result<()> {
    println!("\nRunning Bazel build...");

    let status = Command::new("bazel")
        .args(["build", "//..."])
        .current_dir(project_dir)
        .status()
        .context("Failed to execute bazel build")?;

    if !status.success() {
        anyhow::bail!("Bazel build failed");
    }

    Ok(())
}

fn find_app_target(graph: &TuistGraph, target_hint: Option<&str>) -> Result<(String, String)> {
    // Parse projects array to find app targets
    if let Some(projects_array) = graph.projects.as_array() {
        for item in projects_array {
            if item.as_object().is_some() {
                let project: TuistProject = serde_json::from_value(item.clone())
                    .context("Failed to parse project from graph")?;

                for (key, target) in &project.targets {
                    if target.product == "app" {
                        // If target hint provided, match it
                        if let Some(hint) = target_hint {
                            if key.to_lowercase() == hint.to_lowercase() {
                                return Ok((key.to_lowercase(), target.bundle_id.clone()));
                            }
                        } else {
                            // Return first app target found
                            return Ok((key.to_lowercase(), target.bundle_id.clone()));
                        }
                    }
                }
            }
        }
    }

    anyhow::bail!("No app target found in project")
}

fn run_in_simulator(
    project_dir: &Path,
    target_name: &str,
    bundle_id: &str,
    simulator: &str,
) -> Result<()> {
    println!("\n=== Launching App in Simulator ===");

    // Build the specific target with Bazel
    println!("Building target: {}", target_name);
    let build_status = Command::new("bazel")
        .args(["build", &format!(":{}", target_name)])
        .current_dir(project_dir)
        .status()
        .context("Failed to build target with Bazel")?;

    if !build_status.success() {
        anyhow::bail!("Bazel build failed for target {}", target_name);
    }

    // Boot simulator (ignore errors if already booted)
    println!("Booting simulator: {}", simulator);
    let _ = Command::new("xcrun")
        .args(["simctl", "boot", simulator])
        .status();

    // Wait a moment for simulator to boot
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Get IPA path
    let ipa_path = project_dir.join(format!("bazel-bin/{}.ipa", target_name));

    if !ipa_path.exists() {
        anyhow::bail!("IPA not found at: {}", ipa_path.display());
    }

    // Install the app
    println!("Installing app: {}", ipa_path.display());
    let install_status = Command::new("xcrun")
        .args(["simctl", "install", "booted", ipa_path.to_str().unwrap()])
        .status()
        .context("Failed to install app on simulator")?;

    if !install_status.success() {
        anyhow::bail!("Failed to install app on simulator");
    }

    // Launch the app
    println!("Launching app: {}", bundle_id);
    let launch_output = Command::new("xcrun")
        .args(["simctl", "launch", "booted", bundle_id])
        .output()
        .context("Failed to launch app")?;

    if !launch_output.status.success() {
        anyhow::bail!(
            "Failed to launch app: {}",
            String::from_utf8_lossy(&launch_output.stderr)
        );
    }

    let output_str = String::from_utf8_lossy(&launch_output.stdout);
    println!("\n✓ App launched successfully!");
    println!("Process ID: {}", output_str.trim());
    println!("\nTip: Open Simulator.app to see the running app");

    Ok(())
}
