export type UnityConflictChoice = "ours" | "theirs" | "manual" | "fields";

export type UnityConflictResolution = {
	choice: UnityConflictChoice;
	manualText?: string;
	fields?: Record<string, UnityConflictFieldResolution>;
};

export type UnityConflictFieldResolution = {
	choice: UnityConflictChoice;
	manualText?: string;
};

export type UnityConflictField = {
	id: string;
	label: string;
	ours: string;
	theirs: string;
};

export type UnityConflictBlock = {
	id: string;
	label: string;
	context: string;
	ours: string;
	theirs: string;
	fields: UnityConflictField[];
};

type UnityConflictSection =
	| {
			type: "text";
			content: string;
	  }
	| {
			type: "conflict";
			block: UnityConflictBlock;
	  };

export type UnityConflictDocument = {
	path: string;
	blocks: UnityConflictBlock[];
	sections: UnityConflictSection[];
};

const UNITY_YAML_EXTENSIONS = new Set([
	".anim",
	".asset",
	".controller",
	".mat",
	".overridecontroller",
	".playable",
	".prefab",
	".unity",
]);

export function isUnityYamlPath(path: string): boolean {
	const lowerPath = path.toLowerCase();
	return Array.from(UNITY_YAML_EXTENSIONS).some((extension) => lowerPath.endsWith(extension));
}

export function isUnityYamlConflictFile(path: string, content: string): boolean {
	return isUnityYamlPath(path) && content.includes("<<<<<<<");
}

export function parseUnityConflictDocument(
	path: string,
	content: string,
): UnityConflictDocument | null {
	if (!isUnityYamlConflictFile(path, content)) return null;

	const sections: UnityConflictSection[] = [];
	const blocks: UnityConflictBlock[] = [];
	const lines = content.match(/[^\n]*\n|[^\n]+$/g) ?? [];
	let plainText = "";
	let cursor = 0;
	let blockIndex = 0;

	while (cursor < lines.length) {
		const line = lines[cursor];
		if (!line?.startsWith("<<<<<<<")) {
			plainText += line ?? "";
			cursor += 1;
			continue;
		}

		const leadingText = plainText;
		if (leadingText) {
			sections.push({ type: "text", content: leadingText });
			plainText = "";
		}

		cursor += 1;
		const ours: string[] = [];
		while (cursor < lines.length && !lines[cursor]?.startsWith("=======")) {
			ours.push(lines[cursor] ?? "");
			cursor += 1;
		}
		if (cursor >= lines.length) return null;

		cursor += 1;
		const theirs: string[] = [];
		while (cursor < lines.length && !lines[cursor]?.startsWith(">>>>>>>")) {
			theirs.push(lines[cursor] ?? "");
			cursor += 1;
		}
		if (cursor >= lines.length) return null;
		cursor += 1;

		blockIndex += 1;
		const block: UnityConflictBlock = {
			id: `conflict-${blockIndex}`,
			label: inferConflictLabel(leadingText, ours.join(""), theirs.join(""), blockIndex),
			context: inferConflictContext(leadingText),
			ours: ours.join(""),
			theirs: theirs.join(""),
			fields: inferConflictFields(ours.join(""), theirs.join("")),
		};
		blocks.push(block);
		sections.push({ type: "conflict", block });
	}

	if (plainText) {
		sections.push({ type: "text", content: plainText });
	}

	return {
		path,
		blocks,
		sections,
	};
}

export function applyUnityConflictResolutions(
	document: UnityConflictDocument,
	resolutions: Record<string, UnityConflictResolution>,
): string {
	return document.sections
		.map((section) => {
			if (section.type === "text") {
				return section.content;
			}

			const resolution = resolutions[section.block.id];
			if (!resolution) {
				throw new Error(`Missing resolution for ${section.block.id}`);
			}

			switch (resolution.choice) {
				case "ours":
					return section.block.ours;
				case "theirs":
					return section.block.theirs;
				case "manual":
					return resolution.manualText ?? "";
				case "fields":
					return applyFieldResolutions(section.block, resolution.fields ?? {});
			}
		})
		.join("");
}

function inferConflictLabel(
	plainText: string,
	ours: string,
	theirs: string,
	blockIndex: number,
): string {
	const propertyLine = [...ours.split(/\r?\n/), ...theirs.split(/\r?\n/)]
		.map((line) => line.trim())
		.find((line) => /^[A-Za-z0-9_]+:/.test(line));
	if (propertyLine) {
		return propertyLine.split(":")[0] ?? `Conflict ${blockIndex}`;
	}

	const contextLine = lastMeaningfulLine(plainText);
	return contextLine ? truncate(contextLine, 72) : `Conflict ${blockIndex}`;
}

function inferConflictFields(ours: string, theirs: string): UnityConflictField[] {
	const oursFields = splitUnityFieldGroups(ours);
	const theirsFields = splitUnityFieldGroups(theirs);
	const labels = [...new Set([...oursFields.keys(), ...theirsFields.keys()])];
	if (labels.length <= 1) return [];

	return labels.map((label, index) => ({
		id: `field-${index + 1}`,
		label,
		ours: oursFields.get(label) ?? "",
		theirs: theirsFields.get(label) ?? "",
	}));
}

function splitUnityFieldGroups(content: string): Map<string, string> {
	const groups = new Map<string, string>();
	const lines = content.match(/[^\n]*\n|[^\n]+$/g) ?? [];
	let currentLabel: string | undefined;
	let currentLines: string[] = [];

	function flush() {
		if (!currentLabel) return;
		groups.set(currentLabel, currentLines.join(""));
	}

	for (const line of lines) {
		const label = unityFieldLabel(line);
		if (label) {
			flush();
			currentLabel = label;
			currentLines = [line];
		} else if (currentLabel) {
			currentLines.push(line);
		}
	}
	flush();

	return groups;
}

function unityFieldLabel(line: string): string | undefined {
	const match = line.match(/^\s*([A-Za-z0-9_]+):/);
	const label = match?.[1];
	if (!label) return undefined;
	if (label === "fileID" || label === "guid" || label === "type") return undefined;
	return label;
}

function applyFieldResolutions(
	block: UnityConflictBlock,
	fields: Record<string, UnityConflictFieldResolution>,
): string {
	return block.fields
		.map((field) => {
			const resolution = fields[field.id];
			if (!resolution) {
				throw new Error(`Missing field resolution for ${field.label}`);
			}
			switch (resolution.choice) {
				case "ours":
					return field.ours;
				case "theirs":
					return field.theirs;
				case "manual":
					return resolution.manualText ?? "";
				case "fields":
					return "";
			}
		})
		.join("");
}

function inferConflictContext(plainText: string): string {
	const lines = plainText
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter((line) => line.length > 0);

	let unityObjectName: string | undefined;
	for (let index = lines.length - 1; index >= 0; index -= 1) {
		const line = lines[index];
		if (!line) continue;
		if (/^[A-Za-z][A-Za-z0-9_]+:$/.test(line) && !line.startsWith("m_")) {
			unityObjectName = line.replace(/:$/, "");
			break;
		}
	}

	const contextLine = lastMeaningfulLine(plainText);
	return [unityObjectName, contextLine].filter(Boolean).join(" • ") || "Unity YAML conflict";
}

function lastMeaningfulLine(plainText: string): string | undefined {
	const lines = plainText
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(
			(line) =>
				line.length > 0 &&
				!line.startsWith("%YAML") &&
				!line.startsWith("%TAG") &&
				!line.startsWith("--- !u!"),
		);
	return lines.at(-1);
}

function truncate(value: string, maxLength: number): string {
	if (value.length <= maxLength) return value;
	return `${value.slice(0, maxLength - 1)}…`;
}
