/** @type {import('tailwindcss').Config} */
module.exports = {
    content: {
        files: ["*.html", "./src/**/*.rs"],
    },
    theme: {
        extend: {
            colors: {
                slate: {
                    950: '#020617',
                }
            },
        },
    },
    plugins: [],
}
