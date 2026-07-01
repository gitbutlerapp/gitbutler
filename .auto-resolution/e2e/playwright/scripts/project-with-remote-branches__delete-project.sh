#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "PROJECT NAME: $1"

project_path="$(cd "$1" && pwd)"
projects_file="${E2E_TEST_APP_DATA_DIR}/com.gitbutler.app/projects.json"

if [ -f "$projects_file" ]; then
  node - "$projects_file" "$project_path" <<'NODE'
const fs = require("node:fs");

const [projectsFile, projectPath] = process.argv.slice(2);
const resolvedProjectPath = fs.realpathSync(projectPath);
const projects = JSON.parse(fs.readFileSync(projectsFile, "utf8"));

const nextProjects = projects.filter((project) => {
  const storedPath = project.path;
  if (!storedPath) {
    return true;
  }

  try {
    return fs.realpathSync(storedPath) !== resolvedProjectPath;
  } catch {
    return true;
  }
});

fs.writeFileSync(projectsFile, `${JSON.stringify(nextProjects, null, 2)}\n`);
NODE
fi
