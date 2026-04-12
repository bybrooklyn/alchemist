# Todo List

## AMD / VAAPI / AMF

- Validate `av1_vaapi` on real Linux VAAPI hardware — confirm encode succeeds with current args.
- Validate `av1_amf` on real Windows AMF hardware — confirm encode succeeds with current args.
- If either encoder needs quality/rate-control params, apply the same pattern as the VideoToolbox fix (add `rate_control: Option<&RateControl>` to `vaapi::append_args` and `amf::append_args`).
- Update support claims in README and UI only after validation passes.
