import {
	applyUnityConflictResolutions,
	isUnityYamlConflictFile,
	parseUnityConflictDocument,
} from "$lib/files/unityConflicts";
import { describe, expect, test } from "vitest";

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

describe("unityConflicts", () => {
	test("detects Unity YAML conflict files", () => {
		expect(isUnityYamlConflictFile("Assets/Scenes/dealers.unity", SCENE_CONFLICT)).toBe(true);
		expect(isUnityYamlConflictFile("notes.txt", SCENE_CONFLICT)).toBe(false);
		expect(isUnityYamlConflictFile("Assets/Scenes/dealers.unity", "clean scene")).toBe(false);
	});

	test("parses conflict blocks with Unity context", () => {
		const document = parseUnityConflictDocument("Assets/Scenes/dealers.unity", SCENE_CONFLICT);

		expect(document).not.toBeNull();
		expect(document?.blocks).toHaveLength(1);
		expect(document?.blocks[0]?.label).toContain("m_TagString");
		expect(document?.blocks[0]?.context).toContain("GameObject");
		expect(document?.blocks[0]?.ours).toContain("DealerLocal");
		expect(document?.blocks[0]?.theirs).toContain("DealerUpstream");
	});

	test("applies selected resolutions back into the scene", () => {
		const document = parseUnityConflictDocument("Assets/Scenes/dealers.unity", SCENE_CONFLICT);
		expect(document).not.toBeNull();

		const resolved = applyUnityConflictResolutions(document!, {
			[document!.blocks[0]!.id]: { choice: "theirs" },
		});

		expect(resolved).not.toContain("<<<<<<<");
		expect(resolved).toContain("DealerUpstream");
		expect(resolved).not.toContain("DealerLocal");
	});
});
