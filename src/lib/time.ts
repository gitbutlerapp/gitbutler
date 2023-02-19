export const toHumanReadableTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleTimeString("en-US", {
        hour: "numeric",
        minute: "numeric",
    });
};

export const toHumanReadableDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString("en-US", {
        dateStyle: "short",
    });
};
