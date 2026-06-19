use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VcdError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

fn vcd_code(idx: usize) -> String {
    const CHARS: &[u8] =
        b"!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    let n = CHARS.len();
    if idx < n {
        (CHARS[idx] as char).to_string()
    } else {
        let hi = (idx - n) / n;
        let lo = (idx - n) % n;
        format!("{}{}", CHARS[hi] as char, CHARS[lo] as char)
    }
}

fn write_vcd_value(w: &mut impl Write, code: &str, aval: u64, bval: u64, width: u32) -> std::io::Result<()> {
    if width == 1 {
        let ch = match (aval & 1, bval & 1) {
            (0, 0) => '0',
            (1, 0) => '1',
            (0, 1) => 'z',
            _ => 'x',
        };
        writeln!(w, "{}{}", ch, code)
    } else {
        write!(w, "b")?;
        for bit in (0..width.min(64)).rev() {
            let a = (aval >> bit) & 1;
            let b = (bval >> bit) & 1;
            let ch = match (a, b) {
                (0, 0) => '0',
                (1, 0) => '1',
                (0, 1) => 'z',
                _ => 'x',
            };
            write!(w, "{}", ch)?;
        }
        writeln!(w, " {}", code)
    }
}

fn write_scope_dfs(
    w: &mut impl Write,
    scope_id: u32,
    children_map: &HashMap<u32, Vec<u32>>,
    scope_names: &HashMap<u32, &str>,
    nets_by_scope: &HashMap<u32, Vec<(String, u32, String)>>,
) -> std::io::Result<()> {
    let name = scope_names.get(&scope_id).copied().unwrap_or("unknown");
    writeln!(w, "$scope module {} $end", name)?;
    if let Some(nets) = nets_by_scope.get(&scope_id) {
        for (code, width, net_name) in nets {
            writeln!(w, "$var wire {} {} {} $end", width, code, net_name)?;
        }
    }
    if let Some(kids) = children_map.get(&scope_id) {
        for &kid in kids {
            write_scope_dfs(w, kid, children_map, scope_names, nets_by_scope)?;
        }
    }
    writeln!(w, "$upscope $end")
}

pub struct VcdWriter {
    writer: BufWriter<File>,
    code_map: HashMap<u32, (String, u32)>,   // net_id → (code, width)
    pending: HashMap<u32, (u64, u64)>,        // net_id → (aval, bval)
    last_time: u64,
}

impl VcdWriter {
    /// Create writer and emit VCD header.
    ///
    /// `scopes`: slice of `(scope_id, parent_scope_id, name)`; root scopes have `parent = None`
    /// encoded as `u32::MAX`.
    /// `nets`: slice of `(net_id, scope_id, name, width)`.
    /// `roots`: scope_ids that have no parent.
    pub fn new(
        path: &Path,
        scopes: &[(u32, u32, &str)],  // (id, parent_or_MAX, name)
        nets: &[(u32, u32, &str, u32)], // (net_id, scope_id, name, width)
        root_scope_ids: &[u32],
    ) -> Result<Self, VcdError> {
        let file = File::create(path)?;
        let mut w = BufWriter::new(file);

        writeln!(w, "$version rverilog v0.1.0 $end")?;
        writeln!(w, "$timescale 1ns $end")?;

        // Build scope lookup
        let scope_names: HashMap<u32, &str> = scopes.iter().map(|&(id, _, name)| (id, name)).collect();
        let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(id, parent, _) in scopes {
            if parent != u32::MAX {
                children_map.entry(parent).or_default().push(id);
            }
        }

        // Assign VCD codes to nets in order
        let mut code_map: HashMap<u32, (String, u32)> = HashMap::new();
        let mut nets_by_scope: HashMap<u32, Vec<(String, u32, String)>> = HashMap::new();
        for (i, &(net_id, scope_id, name, width)) in nets.iter().enumerate() {
            let code = vcd_code(i);
            code_map.insert(net_id, (code.clone(), width));
            nets_by_scope
                .entry(scope_id)
                .or_default()
                .push((code, width, name.to_string()));
        }

        // Write scope/var hierarchy
        for &root_id in root_scope_ids {
            write_scope_dfs(&mut w, root_id, &children_map, &scope_names, &nets_by_scope)?;
        }

        writeln!(w, "$enddefinitions $end")?;

        Ok(VcdWriter {
            writer: w,
            code_map,
            pending: HashMap::new(),
            last_time: 0,
        })
    }

    /// Write $dumpvars with initial values (called once before simulation).
    pub fn dump_initial(&mut self, values: &HashMap<u32, (u64, u64)>) -> Result<(), VcdError> {
        writeln!(self.writer, "$dumpvars")?;
        let pairs: Vec<_> = self.code_map.iter()
            .map(|(&id, (code, width))| (id, code.clone(), *width))
            .collect();
        for (id, code, width) in &pairs {
            let (aval, bval) = values.get(id).copied().unwrap_or((0, 0));
            write_vcd_value(&mut self.writer, code, aval, bval, *width)?;
        }
        writeln!(self.writer, "$end")?;
        Ok(())
    }

    /// Record a net value change at the current simulation time.
    pub fn record_change(&mut self, net_id: u32, aval: u64, bval: u64) {
        if self.code_map.contains_key(&net_id) {
            self.pending.insert(net_id, (aval, bval));
        }
    }

    /// Flush pending changes (tagged with `current_time`), then set internal time to `new_time`.
    pub fn advance_time(&mut self, current_time: u64, new_time: u64) -> Result<(), VcdError> {
        self.flush_at(current_time)?;
        self.last_time = new_time;
        Ok(())
    }

    /// Flush any remaining pending changes at the given time.
    pub fn flush_at(&mut self, t: u64) -> Result<(), VcdError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        writeln!(self.writer, "#{}", t)?;
        let pending = std::mem::take(&mut self.pending);
        let code_map = &self.code_map;
        for (net_id, (aval, bval)) in &pending {
            if let Some((code, width)) = code_map.get(net_id) {
                write_vcd_value(&mut self.writer, code, *aval, *bval, *width)?;
            }
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), VcdError> {
        self.flush_at(self.last_time)?;
        self.writer.flush()?;
        Ok(())
    }
}
