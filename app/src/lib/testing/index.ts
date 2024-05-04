import { mockIPC } from "@tauri-apps/api/mocks";
import { mockWindows } from '@tauri-apps/api/mocks';
import { baseBranch, project, user } from "./fixtures"

export const mockTauri = () => {

    // TODO: Set localSTorage like 'lastProject'
    mockIPC((cmd, args) => {
        console.log("MOCKIPC.CMD", cmd, args)
        // console.log(JSON.stringify(args, null, 2))
        // console.groupEnd()
        // Open Project Dialog
        if (cmd === "tauri" && args.__tauriModule === "Dialog" && args.message?.cmd === "openDialog") {
            return "/Users/user/project"
        }

        // List Projects
        if (cmd === "list_projects") {

            console.log('mock.projects', [project])
            return [project]
        }

        // List Project
        if (cmd === "get_project" && args.id === "abc123") {
            return project
        }

        // Get HEAD
        if (cmd === "git_head") {
            return "refs/heads/abc123"
        }

        if (cmd === "menu_item_set_enabled") {
            return true
        }

        if (cmd === "fetch_from_target") {
            return true
        }

        if (cmd === "get_base_branch_data") {
            console.log('mock.baseBranch', baseBranch)
            return baseBranch
        }

        if (cmd === "get_user") {
            return user
        }
    });
    mockWindows('main');

}
