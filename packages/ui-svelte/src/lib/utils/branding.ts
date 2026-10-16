export type Branded<T, Brand extends string> = T & { readonly __brand: Brand };

export type BrandedId<Brand extends string> = Branded<string, Brand>;
