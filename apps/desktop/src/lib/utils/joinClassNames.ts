export function joinClassNames(classNames: string[]): string {
	return classNames.filter(Boolean).join(' ');
}
