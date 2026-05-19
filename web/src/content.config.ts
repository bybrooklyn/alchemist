import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";

const help = defineCollection({
    loader: glob({ pattern: "**/*.md", base: "./src/content/help" }),
    schema: z.object({
        title: z.string(),
        summary: z.string(),
        area: z.enum(["quality", "transcoding", "notifications", "operations"]),
        order: z.number().int().nonnegative().default(0),
    }),
});

export const collections = { help };
