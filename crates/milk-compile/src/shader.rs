use anyhow::Result;
pub use effect_model::shader::{Binding, Shader};

pub fn from_spirv(bytes: &[u8]) -> Result<Shader> {
    let module = naga::front::spv::parse_u8_slice(
        bytes,
        &naga::front::spv::Options {
            adjust_coordinate_space: false,
            ..Default::default()
        },
    )?;
    portable(module)
}
pub fn from_wgsl(source: &str) -> Result<Shader> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| anyhow::anyhow!(e.emit_to_string(source)))?;
    portable(module)
}
fn portable(mut module: naga::Module) -> Result<Shader> {
    let mut bindings = vec![];
    let mut next = 0;
    // Combined SPIR-V sampled images become distinct image/sampler globals.
    // Assign portable unique bindings from the compiler's reflected resources.
    for (_, variable) in module.global_variables.iter_mut() {
        if variable.binding.is_none() {
            continue;
        }
        let kind = match module.types[variable.ty].inner {
            naga::TypeInner::Image {
                dim: naga::ImageDimension::D3,
                ..
            } => "texture3d",
            naga::TypeInner::Image { .. } => "texture2d",
            naga::TypeInner::Sampler { .. } => "sampler",
            _ => "uniform",
        };
        variable.binding = Some(naga::ResourceBinding {
            group: 0,
            binding: next,
        });
        bindings.push(Binding {
            group: 0,
            binding: next,
            name: variable.name.clone().unwrap_or_default(),
            kind: kind.into(),
            resource: (kind != "uniform").then(|| {
                effect_model::shader::resource_alias(variable.name.as_deref().unwrap_or_default())
            }),
        });
        next += 1;
    }
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)?;
    let entry = module
        .entry_points
        .first()
        .ok_or_else(|| anyhow::anyhow!("shader entry point missing"))?
        .name
        .clone();
    let wgsl = naga::back::wgsl::write_string(
        &module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )?;
    let parsed = naga::front::wgsl::parse_str(&wgsl)
        .map_err(|e| anyhow::anyhow!(e.emit_to_string(&wgsl)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&parsed)?;
    Ok(Shader {
        wgsl,
        bindings,
        entry,
        texture_sizes: vec![],
        loop_budget: None,
    })
}

pub fn bounded_wgsl(source: &str, budget: u32) -> Result<Shader> {
    anyhow::ensure!(
        (1..=256).contains(&budget),
        "shader iteration budget must be 1..256"
    );
    let mut module = naga::front::wgsl::parse_str(source)
        .map_err(|e| anyhow::anyhow!(e.emit_to_string(source)))?;
    let span = naga::Span::UNDEFINED;
    let ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(naga::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            }),
        },
        span,
    );
    let zero = module
        .global_expressions
        .append(naga::Expression::Literal(naga::Literal::U32(0)), span);
    let counter = module.global_variables.append(
        naga::GlobalVariable {
            name: Some("poc_iterations".into()),
            space: naga::AddressSpace::Private,
            binding: None,
            ty,
            init: Some(zero),
            memory_decorations: Default::default(),
        },
        span,
    );
    for (_, function) in module.functions.iter_mut() {
        cap_loops(
            &mut function.body,
            &mut function.expressions,
            counter,
            budget,
        );
    }
    for entry in &mut module.entry_points {
        cap_loops(
            &mut entry.function.body,
            &mut entry.function.expressions,
            counter,
            budget,
        );
    }
    let mut result = portable(module)?;
    result.loop_budget = Some(budget);
    Ok(result)
}

fn cap_loops(
    block: &mut naga::Block,
    expressions: &mut naga::Arena<naga::Expression>,
    counter: naga::Handle<naga::GlobalVariable>,
    budget: u32,
) {
    use naga::{BinaryOperator as B, Expression as E, Literal as L, Statement as S};
    let span = naga::Span::UNDEFINED;
    for (statement, _) in block.span_iter_mut() {
        match statement {
            S::Block(body) => cap_loops(body, expressions, counter, budget),
            S::If { accept, reject, .. } => {
                cap_loops(accept, expressions, counter, budget);
                cap_loops(reject, expressions, counter, budget);
            }
            S::Switch { cases, .. } => {
                for case in cases {
                    cap_loops(&mut case.body, expressions, counter, budget);
                }
            }
            S::Loop {
                body, continuing, ..
            } => {
                cap_loops(body, expressions, counter, budget);
                cap_loops(continuing, expressions, counter, budget);
                let pointer = expressions.append(E::GlobalVariable(counter), span);
                let one = expressions.append(E::Literal(L::U32(1)), span);
                let limit = expressions.append(E::Literal(L::U32(budget)), span);
                let load = expressions.append(E::Load { pointer }, span);
                let condition = expressions.append(
                    E::Binary {
                        op: B::GreaterEqual,
                        left: load,
                        right: limit,
                    },
                    span,
                );
                let increment = expressions.append(
                    E::Binary {
                        op: B::Add,
                        left: load,
                        right: one,
                    },
                    span,
                );
                let prefix = naga::Block::from_vec(vec![
                    S::Emit(naga::Range::new_from_bounds(load, increment)),
                    S::If {
                        condition,
                        accept: naga::Block::from_vec(vec![S::Break]),
                        reject: naga::Block::new(),
                    },
                    S::Store {
                        pointer,
                        value: increment,
                    },
                ]);
                body.splice(0..0, prefix);
            }
            _ => {}
        }
    }
}
