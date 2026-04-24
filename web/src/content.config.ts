import { defineCollection, z } from "astro:content";

const help = defineCollection({
    type: "content",
    schema: z.object({
        title: z.string(),
        summary: z.string(),
        area: z.enum(["quality", "transcoding", "notifications", "operations"]),
        order: z.number().int().nonnegative().default(0),
    }),
});

export const collections = { help };
