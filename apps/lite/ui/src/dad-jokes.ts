import dadJokes from "./dad-jokes.json";

type DadJoke = (typeof dadJokes)[number];

export const getRandomDadJoke = (): DadJoke =>
	// oxlint-disable-next-line typescript/no-non-null-assertion -- The file is not empty.
	dadJokes[Math.floor(Math.random() * dadJokes.length)]!;
