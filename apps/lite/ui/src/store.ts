import { ProjectsStore } from "#ui/projects/ProjectsStore.ts";
import { configure } from "mobx";
import { createContext, useContext } from "react";

configure({ enforceActions: "always" });

export const projectsStore = new ProjectsStore();
export const StoreContext = createContext<ProjectsStore | null>(null);

export const useProjectsStore = () => {
	const store = useContext(StoreContext);
	if (!store) throw new Error("Store hooks must be used within StoreContext");
	return store;
};

export const useProjectStore = (projectId: string) => useProjectsStore().getProject(projectId);
