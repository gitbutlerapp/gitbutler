export type Week = {
    start: Date;
    end: Date;
};
export namespace Week {
    export const from = (date: Date): Week => {
        const start = new Date(date);
        start.setHours(0, 0, 0, 0);
        start.setDate(start.getDate() - start.getDay() + 1); // Start on Monday
        const end = new Date(start);
        end.setDate(end.getDate() + 7);
        return { start, end };
    };
    export const next = (week: Week): Week => {
        const start = new Date(week.start);
        start.setDate(start.getDate() + 7);
        const end = new Date(week.end);
        end.setDate(end.getDate() + 7);
        return { start, end };
    };
    export const previous = (week: Week): Week => {
        const start = new Date(week.start);
        start.setDate(start.getDate() - 7);
        const end = new Date(week.end);
        end.setDate(end.getDate() - 7);
        return { start, end };
    };
    export const nThDay = (week: Week, n: number): Date => {
        const date = new Date(week.start);
        date.setDate(date.getDate() + n);
        return date;
    };
}
