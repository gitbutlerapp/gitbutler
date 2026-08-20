const knownRasterImageExts = new Set([
	"png",
	"jpg",
	"jpeg",
	"gif",
	"webp",
	"bmp",
	"ico",
	"heic",
	"heif",
	"avif",
]);

const getFileExtension = (path: string): string => {
	const fileName = path.slice(path.lastIndexOf("/") + 1);
	const dotIndex = fileName.lastIndexOf(".");
	return dotIndex === -1 ? "" : fileName.slice(dotIndex + 1).toLowerCase();
};

export const isRasterImageFile = (path: string): boolean =>
	knownRasterImageExts.has(getFileExtension(path));

export const isSvgFile = (path: string): boolean => getFileExtension(path) === "svg";
