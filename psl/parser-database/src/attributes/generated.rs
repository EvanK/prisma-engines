use crate::{
    ast::{self, WithName},
    coerce,
    context::Context,
    types::{GeneratedAttribute, ScalarFieldType},
    DatamodelError, ScalarFieldId, StringId,
};

/// @generated on model scalar fields
pub(super) fn visit_model_field_generated(
    scalar_field_id: ScalarFieldId,
    model_id: crate::ModelId,
    field_id: ast::FieldId,
    r#type: ScalarFieldType,
    ctx: &mut Context<'_>,
) {
    let (argument_idx, value) = match ctx.visit_default_arg_with_idx("value") {
        Ok(value) => value,
        Err(err) => return ctx.push_error(err),
    };

    let ast_model = &ctx.asts[model_id];
    let ast_field = &ast_model[field_id];

    // let mapped_name = default_attribute_mapped_name(ctx);
    let kind_name = generated_attribute_kind_name(ctx);
    let generated_attribute_id = ctx.current_attribute_id();

    let mut accept = move |ctx: &mut Context<'_>| {
        let generated_value = GeneratedAttribute {
            argument_idx,
            kind_name,
            generated_attribute: generated_attribute_id,
        };

        ctx.types[scalar_field_id].generated = Some(generated_value);
    };

    match r#type {
        // composite types should fail in the same way as @default
        ScalarFieldType::CompositeType(ctid) => {
            validate_generated_value_on_composite_type(ctid, ast_field, ctx);
        }
        // enums we can only validate as far as constants (or arrays thereof);
        // anything else is acceptable until the db itself evaluates the generate value expr
        ScalarFieldType::Enum(enum_id) => {
            if ast_field.arity.is_list() {
                validate_enum_list_generated(value, enum_id, &mut accept, ctx);
            } else {
                validate_enum_generated(value, enum_id, &mut accept, ctx);
            }
        }
        // any scalar type should be acceptable
        ScalarFieldType::BuiltInScalar(_) => {
            accept(ctx)
        }
        // Unsupported type should still be acceptable
        ScalarFieldType::Unsupported(_) => {
            accept(ctx)
        }
    }
}

/// @generated on composite type fields
pub(super) fn visit_composite_field_generated(
    ct_id: crate::CompositeTypeId,
    field_id: ast::FieldId,
    r#type: ScalarFieldType,
    ctx: &mut Context<'_>,
) {
    let (argument_idx, value) = match ctx.visit_default_arg_with_idx("value") {
        Ok(value) => value,
        Err(err) => return ctx.push_error(err),
    };

    let ast_model = &ctx.asts[ct_id];
    let ast_field = &ast_model[field_id];

    let kind_name = generated_attribute_kind_name(ctx);

    let generated_attribute = ctx.current_attribute_id();

    let mut accept = move |ctx: &mut Context<'_>| {
        let generated_value = GeneratedAttribute {
            argument_idx,
            kind_name,
            generated_attribute,
        };

        let field_data = ctx.types.composite_type_fields.get_mut(&(ct_id, field_id)).unwrap();
        field_data.generated = Some(generated_value);
    };

    // Resolve the default to a DefaultValue. We must loop in order to
    // resolve type aliases.
    match r#type {
        ScalarFieldType::CompositeType(ctid) => {
            validate_generated_value_on_composite_type(ctid, ast_field, ctx);
        }
        ScalarFieldType::Enum(enum_id) => {
            if ast_field.arity.is_list() {
                validate_enum_list_generated(value, enum_id, &mut accept, ctx);
            } else {
                validate_enum_generated(value, enum_id, &mut accept, ctx);
            }
        }
        ScalarFieldType::BuiltInScalar(_) => {
            accept(ctx)
        }
        ScalarFieldType::Unsupported(_) => {
            accept(ctx)
        }
    }
}

fn generated_attribute_kind_name(ctx: &mut Context<'_>) -> Option<StringId> {
    match ctx
        .visit_optional_arg("kind")
        .and_then(|name| coerce::string(name, ctx.diagnostics))
    {
        Some(name) => {
            let name_normalized: &str = &name.trim().to_uppercase();
            match name_normalized {
                // allow empty strings as essentially a default for the connector side of things
                "" => None,
                // stored or virtual, accepting any non-standard aliases (eg, mssql uses persistent)
                KIND_STORED | KIND_PERSISTED => Some(ctx.interner.intern(KIND_STORED)),
                KIND_VIRTUAL => Some(ctx.interner.intern(KIND_VIRTUAL)),
                // anything else should push an error message
                _ => {
                    ctx.push_attribute_validation_error(&format!(
                        "Expected a `kind` argument for generated value, but found `{name_normalized}`."
                    ));
                    None
                }
            }
        },
        None => None,
    }
}

fn validate_invalid_generated_enum_value(enum_value: &str, ctx: &mut Context<'_>) {
    ctx.push_attribute_validation_error(&format!(
        "The defined generated value `{enum_value}` is not a valid value of the enum specified for the field."
    ));
}

fn validate_generated_value_on_composite_type(
    ctid: crate::CompositeTypeId,
    ast_field: &ast::Field,
    ctx: &mut Context<'_>,
) {
    let attr = ctx.current_attribute();
    let ct_name = ctx.asts[ctid].name();

    ctx.push_error(DatamodelError::new_composite_type_field_validation_error(
        "Generated values on fields of type composite are not supported. Please remove the `@generated` attribute.",
        ct_name,
        ast_field.name(),
        attr.span,
    ));
}

fn validate_enum_generated(
    found_value: &ast::Expression,
    enum_id: crate::EnumId,
    accept: AcceptFn<'_>,
    ctx: &mut Context<'_>,
) {
    match found_value {
        ast::Expression::ConstantValue(enum_value, _) => {
            if ctx.asts[enum_id].values.iter().any(|v| v.name() == enum_value) {
                accept(ctx)
            } else {
                validate_invalid_generated_enum_value(enum_value, ctx);
            }
        }
        // accept anything else that we can't validate until sql expr gets evaluated by db
        _ => accept(ctx)
    };
}

fn validate_enum_list_generated(
    found_value: &ast::Expression,
    enum_id: crate::EnumId,
    accept: AcceptFn<'_>,
    ctx: &mut Context<'_>,
) {
    match found_value {
        ast::Expression::Array(values, _) => {
            let mut valid = true;
            let mut enum_values = values.iter();
            while let (true, Some(enum_value)) = (valid, enum_values.next()) {
                valid = false;
                validate_enum_generated(
                    enum_value,
                    enum_id,
                    &mut |_| {
                        valid = true;
                    },
                    ctx,
                );
            }

            if valid {
                accept(ctx);
            }
        }
        // accept anything else that we can't validate until sql expr gets evaluated by db
        _ => accept(ctx),
    };
}

const KIND_STORED: &str = "STORED";
const KIND_PERSISTED: &str = "PERSISTED";
const KIND_VIRTUAL: &str = "VIRTUAL";

const KNOWN_KINDS: &[&str] = &[
    KIND_STORED,
    KIND_PERSISTED,
    KIND_VIRTUAL,
];

type AcceptFn<'a> = &'a mut dyn for<'b, 'c> FnMut(&'b mut Context<'c>);
