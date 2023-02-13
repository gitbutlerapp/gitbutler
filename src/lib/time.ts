export const toHumanReadableTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString("en-US", {
        hour: "numeric",
        minute: "numeric",
    });
};
