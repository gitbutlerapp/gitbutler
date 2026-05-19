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

const MULTI_FIELD_CONFLICT = `%YAML 1.1
--- !u!4 &4000
Transform:
<<<<<<< ours
  m_LocalPosition: {x: 1, y: 2, z: 3}
  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}
  m_Script: {fileID: 11500000, guid: localguid, type: 3}
=======
  m_LocalPosition: {x: 9, y: 9, z: 9}
  m_LocalRotation: {x: 0, y: 1, z: 0, w: 0}
  m_Script: {fileID: 11500000, guid: theirsGuid, type: 3}
>>>>>>> theirs
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

	test("supports resolving Unity fields separately inside one conflict block", async () => {
		const document = parseUnityConflictDocument(
			"Assets/Scenes/dealers.unity",
			MULTI_FIELD_CONFLICT,
		);
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

		await user.click(
			screen.getByRole("button", { name: "Resolve fields separately for conflict 1" }),
		);
		await user.click(screen.getByRole("button", { name: "Use ours for m_LocalPosition" }));
		await user.click(screen.getByRole("button", { name: "Use theirs for m_LocalRotation" }));
		await user.click(screen.getByRole("button", { name: "Use theirs for m_Script" }));
		await user.click(screen.getByRole("button", { name: "Apply to scene" }));

		const resolved = onApply.mock.calls[0]?.[0] as string;
		expect(resolved).toContain("m_LocalPosition: {x: 1, y: 2, z: 3}");
		expect(resolved).toContain("m_LocalRotation: {x: 0, y: 1, z: 0, w: 0}");
		expect(resolved).toContain("guid: theirsGuid");
		expect(resolved).not.toContain("<<<<<<<");
	});
});
