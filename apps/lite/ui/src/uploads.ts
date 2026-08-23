/**
 * Attaching files to markdown bodies.
 *
 * Files are uploaded to gitbutler.com and linked by URL, because no forge
 * accepts image bytes through its review API — a body can only ever carry a
 * link. The upload itself runs in the backend, which is where the GitButler
 * account token stays.
 */

import type { Upload } from "@gitbutler/but-sdk";

/**
 * The backend enforces this too, but only after the bytes have crossed IPC —
 * checking here keeps a doomed file from being read and base64-encoded (~1.4x
 * its size) just to be rejected.
 */
export const UPLOAD_SIZE_LIMIT = 10 * 1024 * 1024;

/** The first file too large to upload, if any. */
export const oversizedFile = (files: ReadonlyArray<File>): File | undefined =>
	files.find((file) => file.size > UPLOAD_SIZE_LIMIT);

/** What the file picker offers, matching what the desktop app accepts. */
export const ACCEPTED_FILE_TYPES = ["image/*", "application/*", "text/*", "audio/*", "video/*"];

/** Encode a file for the backend, which takes bytes as base64 over IPC. */
export const toBase64 = async (file: File): Promise<string> => {
	const bytes = new Uint8Array(await file.arrayBuffer());
	// Chunked: spreading megabytes of bytes into one call overflows the stack.
	const CHUNK = 0x8000;
	let binary = "";
	for (let offset = 0; offset < bytes.length; offset += CHUNK)
		binary += String.fromCharCode(...bytes.subarray(offset, offset + CHUNK));

	return btoa(binary);
};

/** Render uploads as the markdown that embeds images and links everything else. */
export const uploadsToMarkdown = (uploads: ReadonlyArray<Upload>): string =>
	uploads
		.map((upload) => `${upload.isImage ? "!" : ""}[${upload.filename}](${upload.url})`)
		.join("\n");

/**
 * The files carried by a paste or a drop, if any.
 *
 * A pasted screenshot arrives as a file with no name of its own, so it is
 * named here rather than uploaded as the empty string.
 */
export const filesFromTransfer = (transfer: DataTransfer | null): Array<File> => {
	if (transfer === null) return [];
	return Array.from(transfer.files).map((file) =>
		file.name === "" ? new File([file], "pasted-image", { type: file.type }) : file,
	);
};
