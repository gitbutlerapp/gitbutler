export function getBackoffByAge(age: number) {
	if (age < 60000) {
		return 10000;
	} else if (age < 600000) {
		return 30000;
	} else if (age < 1200000) {
		return 60000;
	} else if (age < 3600000) {
		return 120000;
	}
	return;
}
