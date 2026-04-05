/* eslint-disable no-console */
import { listProjectsStatelessNapi, stackDetailsNapi, stacksNapi } from "./generated/index.js";

async function main() {
	const projects = await listProjectsStatelessNapi();
	console.log(projects);

	if (projects.length === 0) {
		console.log("No projects found");
	}

	const project = projects.at(0);
	if (!project) throw new Error("The world is wrong");

	const stacks = await stacksNapi(project.id, null);
	for (const stack of stacks) {
		const details = await stackDetailsNapi(project.id, stack.id);
		console.log("This are the details for stack with id: " + stack.id);
		console.log(details);
	}
}

main();
