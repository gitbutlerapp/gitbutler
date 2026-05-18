import UnityConflictWorkbench from "$components/workspace/UnityConflictWorkbench.svelte";
import { parseUnityConflictDocument } from "$lib/files/unityConflicts";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

const SCENE_CONFLICT = `%YAML 1.1
--- !u!1 &1200
GameObject:
  m_Name: Dealer
<<<<<<< ours
  m_TagString: DealerLocal
=======
  m_TagString: DealerUpstream
>>>>>>> theirs
  m_IsActive: 1
`;

describe("UnityConflictWorkbench", () => {
	test("applies per-conflict selections", async () => {
		const document = parseUnityConflictDocument("Assets/Scenes/dealers.unity", SCENE_CONFLICT);
		expect(document).not.toBeNull();

		const onApply = vi.fn();
		const user = userEvent.setup();

		render(UnityConflictWorkbench, {
			props: {
				filePath: "Assets/Scenes/dealers.unity",
				document: document!,
				onApply,
			},
		});

		const applyButton = screen.getByRole("button", { name: "Apply to scene" });
		expect(applyButton).toBeDisabled();

		await user.click(screen.getByRole("button", { name: "Use theirs for conflict 1" }));
		expect(applyButton).toBeEnabled();

		await user.click(applyButton);

		expect(onApply).toHaveBeenCalledWith(expect.stringContaining("DealerUpstream"));
		expect(onApply).not.toHaveBeenCalledWith(expect.stringContaining("<<<<<<<"));
	});

	test("supports manual resolution text", async () => {
		const document = parseUnityConflictDocument("Assets/Scenes/dealers.unity", SCENE_CONFLICT);
		expect(document).not.toBeNull();

		const onApply = vi.fn();
		const user = userEvent.setup();

		render(UnityConflictWorkbench, {
			props: {
				filePath: "Assets/Scenes/dealers.unity",
				document: document!,
				onApply,
			},
		});

		await user.click(screen.getByRole("button", { name: "Manual edit for conflict 1" }));
		const editor = screen.getByLabelText("Manual resolution for conflict 1");
		await user.clear(editor);
		await user.type(editor, "  m_TagString: DealerMerged");
		await user.click(screen.getByRole("button", { name: "Apply to scene" }));

		expect(onApply).toHaveBeenCalledWith(expect.stringContaining("DealerMerged"));
	});
});
