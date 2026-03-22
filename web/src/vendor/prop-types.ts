type Validator = (() => null) & { isRequired: () => null };

function validator(): Validator {
    const fn = (() => null) as Validator;
    fn.isRequired = () => null;
    return fn;
}

const PropTypes = {
    any: validator(),
    array: validator(),
    element: validator(),
    object: validator(),
    oneOfType: () => validator(),
};

export default PropTypes;
