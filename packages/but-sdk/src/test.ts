/* eslint-disable no-console */
import { listProjectsNapi, stackDetailsNapi, stacksNapi } from "./generated";


function main() {
  const projects = listProjectsNapi([]);
  console.log(projects)

  if (projects.length === 0) {
    console.log("No projects found")
  }

  const project = projects.at(0);
  if (!project) throw new Error("The world is wrong");

  const stacks = stacksNapi(project.id, null);
  for (const stack of stacks) {
    const details = stackDetailsNapi(project.id, stack.id)
    console.log("This are the details for stack with id: " + stack.id)
    console.log(details)
  }
}

main()