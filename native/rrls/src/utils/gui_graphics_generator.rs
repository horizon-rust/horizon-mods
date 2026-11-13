/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `GuiGraphicsGenerator.java`.
//!
//! Upstream this is a dev-time ASM `ClassVisitor`: it walks Minecraft's
//! `GuiGraphicsExtractor`, adds a no-op `MethodNode` for every method
//! `DummyGuiGraphics` does not override yet, collects Fabric access-widener
//! entries for private/final members, and emits the generated
//! `DummyGuiGraphics.java` source plus `rrls.accesswidener`. The bytecode
//! reader is the host seam (here: a descriptor list standing in for the
//! class bytes); the visitor's decision logic, the widener entry format,
//! and the source generator are translated exactly.

/// `Opcodes` access flags used below.
pub mod opcodes {
    pub const ACC_PUBLIC: u32 = 0x0001;
    pub const ACC_STATIC: u32 = 0x0008;
    pub const ACC_PRIVATE: u32 = 0x0002;
    pub const ACC_PROTECTED: u32 = 0x0004;
    pub const ACC_FINAL: u32 = 0x0010;
    pub const ACC_SYNTHETIC: u32 = 0x1000;
    pub const ACC_BRIDGE: u32 = 0x0040;
}

/// The fields of an ASM `MethodNode` the visitor touches.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodNode {
    pub access: u32,
    pub name: String,
    pub descriptor: String,
    /// `invisibleAnnotations`: the emitted stubs carry `@Override`.
    pub has_override_annotation: bool,
}

/// A collected access-widener line ("accessible method <owner> <name>
/// <descriptor>" or the extendable variant).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessWidenerEntry(pub String);

/// `GuiGraphicsGenerator extends ClassVisitor`: accumulates stub methods
/// into `output` and widener entries into `access_widener_entries`.
pub struct GuiGraphicsGenerator {
    pub output: Vec<MethodNode>,
    pub owner_name: String,
    pub access_widener_entries: Vec<AccessWidenerEntry>,
    /// Log sink standing in for `Rrls.LOGGER` (a test can observe it).
    pub log: Vec<String>,
}

impl GuiGraphicsGenerator {
    pub fn new(output: Vec<MethodNode>, owner_name: &str) -> Self {
        Self {
            output,
            owner_name: owner_name.to_string(),
            access_widener_entries: Vec::new(),
            log: Vec::new(),
        }
    }

    /// `visitMethod`: static and lambda methods pass through; constructors
    /// pass through; methods already present in the output are skipped with
    /// a log line; everything else gets a stub appended and widener entries
    /// for private/final access.
    pub fn visit_method(&mut self, node: &MethodNode) -> bool {
        if node.name == "<init>" || node.name == "<clinit>" {
            return true; // super.visitMethod(...)
        }

        if (node.access & opcodes::ACC_STATIC) != 0 {
            return true;
        }

        if node.name.contains("lambda$") {
            return true;
        }

        if self.output.iter().any(|m| m.name == node.name && m.descriptor == node.descriptor) {
            self.log.push(format!("Skipping {}{}...", node.name, node.descriptor));
            return true;
        }

        self.log.push(format!("Adding {}{}...", node.name, node.descriptor));
        self.output.push(MethodNode {
            access: node.access,
            name: node.name.clone(),
            descriptor: node.descriptor.clone(),
            has_override_annotation: true, // new AnnotationNode("@Override")
        });

        let is_private = (node.access & opcodes::ACC_PRIVATE) != 0;
        let is_final = (node.access & opcodes::ACC_FINAL) != 0;

        if is_private {
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "accessible method {} {} {}", self.owner_name, node.name, node.descriptor)));
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "extendable method {} {} {}", self.owner_name, node.name, node.descriptor)));
        } else if is_final {
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "extendable method {} {} {}", self.owner_name, node.name, node.descriptor)));
        }

        false // the returned MethodVisitor collects the (empty) body
    }

    /// `main(...)`: walk the target class's methods and emit the source +
    /// access widener. The ASM reader is a supplied descriptor list.
    pub fn generate(&mut self, class_methods: &[MethodNode]) -> (String, String) {
        for m in class_methods {
            self.visit_method(m);
        }
        (generate_java_source(&self.output), generate_access_widener(&self.access_widener_entries))
    }
}

/// `generateJavaSource`: the license header, package, the fixed imports,
/// the class declaration with the INSTANCE singleton, the constructor, and
/// one public override per stub.
pub fn generate_java_source(output: &[MethodNode]) -> String {
    let mut sb = String::new();

    sb.push_str("/*\n");
    sb.push_str(" * Copyright 2023 - 2026 dima_dencep.\n");
    sb.push_str(" *\n");
    sb.push_str(" * Licensed under the Open Software License, Version 3.0 (the \"License\");\n");
    sb.push_str(" * you may not use this file except in compliance with the License.\n");
    sb.push_str(" *\n");
    sb.push_str(" * You may obtain a copy of the License at\n");
    sb.push_str(" *     https://spdx.org/licenses/OSL-3.0.txt\n");
    sb.push_str(" */\n\n");
    sb.push_str("package org.redlance.dima_dencep.mods.rrls.utils;\n\n");
    sb.push_str("import net.minecraft.client.Minecraft;\n");
    sb.push_str("import net.minecraft.client.gui.GuiGraphics;\n");
    sb.push_str("import net.minecraft.client.gui.render.state.GuiRenderState;\n\n");
    sb.push_str("@SuppressWarnings(\"all\")\n");
    sb.push_str("public class DummyGuiGraphics extends GuiGraphicsExtractor {\n");
    sb.push_str("    public static final DummyGuiGraphics INSTANCE = new DummyGuiGraphics();\n\n");
    sb.push_str("    private DummyGuiGraphics() {\n");
    sb.push_str("        super(Minecraft.getInstance(), (GuiRenderState) null, 0, 0);\n");
    sb.push_str("    }\n");

    for method in output {
        sb.push('\n');
        sb.push_str(&generate_method(method));
    }

    sb.push_str("}\n");
    sb
}

/// `generateMethod`: `@Override` when annotated (or when the flag is set
/// for instance methods, mirroring the fallback for non-static methods),
/// visibility widened to public (private/protected/final bits cleared),
/// then `returnType name(descriptor)` with the no-op body.
pub fn generate_method(method: &MethodNode) -> String {
    let has_override = method.has_override_annotation
        || (method.access & opcodes::ACC_STATIC) == 0;

    let mut sb = String::new();
    if has_override {
        sb.push_str("    @Override\n");
    }

    sb.push_str("    ");
    let mut output_access = method.access;
    output_access &= !(opcodes::ACC_PRIVATE | opcodes::ACC_PROTECTED | opcodes::ACC_FINAL);
    output_access |= opcodes::ACC_PUBLIC;
    sb.push_str(&access_to_string(output_access));
    sb.push_str(&format!("{}{} {{\n", method.name, method.descriptor));
    sb.push_str("    }\n");
    sb
}

/// `accessToString`, the flags ASM prints.
pub fn access_to_string(access: u32) -> String {
    let mut s = String::new();
    if access & opcodes::ACC_PUBLIC != 0 { s.push_str("public "); }
    if access & opcodes::ACC_PROTECTED != 0 { s.push_str("protected "); }
    if access & opcodes::ACC_PRIVATE != 0 { s.push_str("private "); }
    if access & opcodes::ACC_FINAL != 0 { s.push_str("final "); }
    if access & opcodes::ACC_STATIC != 0 { s.push_str("static "); }
    s
}

/// The `rrls.accesswidener` file: the v2 header plus one entry per line.
pub fn generate_access_widener(entries: &[AccessWidenerEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut aw = String::from("accessWidener\tv2\tnamed\n\n");
    for e in entries {
        aw.push_str(&format!("{}\n", e.0));
    }
    aw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, desc: &str, access: u32) -> MethodNode {
        MethodNode { access, name: name.into(), descriptor: desc.into(), has_override_annotation: false }
    }

    #[test]
    fn skip_rules_match_upstream() {
        let mut g = GuiGraphicsGenerator::new(vec![], "a/b/GuiGraphicsExtractor");
        // Constructors, static methods, and lambdas pass through untouched.
        assert!(g.visit_method(&node("<init>", "()V", 0)));
        assert!(g.visit_method(&node("<clinit>", "()V", 0)));
        assert!(g.visit_method(&node("x", "()V", opcodes::ACC_STATIC)));
        assert!(g.visit_method(&node("lambda$y$0", "()V", 0)));
        assert!(g.output.is_empty() && g.access_widener_entries.is_empty());
    }

    #[test]
    fn already_present_methods_are_skipped_with_a_log() {
        let existing = vec![node("guiWidth", "()I", 0)];
        let mut g = GuiGraphicsGenerator::new(existing, "owner");
        assert!(g.visit_method(&node("guiWidth", "()I", 0)));
        assert_eq!(g.output.len(), 1);
        assert_eq!(g.log, vec!["Skipping guiWidth()I...".to_string()]);
    }

    #[test]
    fn new_methods_get_stubs_and_wideners() {
        let mut g = GuiGraphicsGenerator::new(vec![], "owner");
        assert!(!g.visit_method(&node("privateMethod", "(I)V", opcodes::ACC_PRIVATE)));
        assert!(!g.visit_method(&node("finalMethod", "()V", opcodes::ACC_FINAL | opcodes::ACC_PROTECTED)));
        assert_eq!(g.output.len(), 2);
        assert!(g.output.iter().all(|m| m.has_override_annotation));
        assert_eq!(g.access_widener_entries, vec![
            AccessWidenerEntry("accessible method owner privateMethod (I)V".into()),
            AccessWidenerEntry("extendable method owner privateMethod (I)V".into()),
            AccessWidenerEntry("extendable method owner finalMethod ()V".into()),
        ]);
        assert_eq!(g.log[0], "Adding privateMethod(I)V...".to_string());
    }

    #[test]
    fn source_carries_header_instance_and_public_overrides() {
        let out = vec![node("guiWidth", "()I", 0)];
        let src = generate_java_source(&out);
        assert!(src.contains("https://spdx.org/licenses/OSL-3.0.txt"));
        assert!(src.contains("public class DummyGuiGraphics extends GuiGraphicsExtractor {"));
        assert!(src.contains("public static final DummyGuiGraphics INSTANCE"));
        assert!(src.contains("super(Minecraft.getInstance(), (GuiRenderState) null, 0, 0);"));
        assert!(src.contains("@Override\n    public guiWidth()I"));
    }

    #[test]
    fn access_widener_file_has_header() {
        let aw = generate_access_widener(&[AccessWidenerEntry("extendable method a b ()V".into())]);
        assert!(aw.starts_with("accessWidener\tv2\tnamed\n\n"));
        assert!(aw.contains("extendable method a b ()V\n"));
        assert_eq!(generate_access_widener(&[]), "");
    }
}
