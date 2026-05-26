/* eslint-disable no-console */
import { headInfo, listProjectsStateless } from "./generated/index.js";

async function main() {
	const projects = await listProjectsStateless();
	console.log(projects);

	if (projects.length === 0) {
		console.log("No projects found");
	}

	const project = projects.at(0);
	if (!project) throw new Error("The world is wrong");

	const info = await headInfo(project.id);
	console.log(info);
}

main();
