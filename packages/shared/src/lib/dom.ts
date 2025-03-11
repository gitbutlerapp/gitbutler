export async function uploadFiles(accept: string, multiple = true): Promise<FileList | null> {
	const input = document.createElement('input');
	input.type = 'file';
	input.accept = accept;
	input.multiple = multiple;

	// Append to document temporarily (some browsers require this)
	input.style.display = 'none';
	document.body.appendChild(input);

	input.click();

	try {
		return await new Promise<FileList | null>((resolve, reject) => {
			input.onchange = () => {
				resolve(input.files);
			};

			// Handle cancel/error cases
			input.onabort = () => resolve(null);
			input.onerror = (e) => reject(e);
		});
	} finally {
		// Ensure cleanup happens even if there's an error
		document.body.removeChild(input);
	}
}
