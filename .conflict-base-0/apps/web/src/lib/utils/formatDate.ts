export function formatDate(dateStr: string): string {
	const date = new Date(dateStr);
	const now = new Date();

	const diff = now.getTime() - date.getTime();
	const diffInDays = diff / (1000 * 3600 * 24);

	if (diffInDays < 1) {
		const diffInHours = diff / (1000 * 3600);

		if (diffInHours < 1) {
			const diffInMinutes = diff / (1000 * 60);

			if (diffInMinutes < 1) {
				const diffInSeconds = diff / 1000;

				return `${Math.floor(diffInSeconds)} seconds ago`;
			}

			return `${Math.floor(diffInMinutes)} minutes ago`;
		}

		return `${Math.floor(diffInHours)} hours ago`;
	}

	if (diffInDays < 30) {
		return `${Math.floor(diffInDays)} days ago`;
	}

	const diffInMonths = diffInDays / 30;

	if (diffInMonths < 12) {
		return `${Math.floor(diffInMonths)} months ago`;
	}

	const diffInYears = diffInMonths / 12;

	return `${Math.floor(diffInYears)} years ago`;
}
