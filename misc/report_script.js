console.log("Report JSON", DATA);

const DETAILS = DATA.benchs[0].builds[0].times !== null;

function format_bench(name) {
    let parts = name.split(":");

    if (parts.length > 1) {
        if (parts[1] == 'debug') {
            parts[1] = `<span class="bench-debug">debug</span>`
        }
        if (parts[1] == 'check') {
            parts[1] = `<span class="bench-check">check</span>`
        }
        if (parts[1] == 'release') {
            parts[1] = `<span class="bench-release">release</span>`
        }
    }

    if (parts.length > 2) {
        if (parts[2] == 'initial') {
            parts[2] = `<span class="bench-incr">initial</span>`
        }
        if (parts[2] == 'unchanged') {
            parts[2] = `<span class="bench-incr">unchanged</span>`
        }
    }

    return `<span class="bench-name">${parts.join(`<span class="bench-colon">:</span>`)}</span>`;
}

function format_time(secs) {
    return `${secs.toFixed(4)}s`;
}

function format_size(bytes) {
    if (bytes >= 1073741824) {
        return (bytes / 1073741824).toFixed(2) + " GiB";
    }
    else if (bytes >= 1048576) {
        return (bytes / 1048576).toFixed(2) + " MiB";
    }
    else if (bytes >= 1024) {
        return (bytes / 1024).toFixed(2) + " KiB";
    }
    else if (bytes > 1) {
        return bytes + " bytes";
    }
    else if (bytes == 1) {
        return bytes + " byte";
    }
    else if (bytes == 0) {
        return "0 bytes";
    }
    throw `unable to convert bytes ${JSON.stringify(bytes)}`;
}

function change(average, first, b) {
    if (!b) {
        return "";
    }
    let change = (average / first - 1) * 100;
    if (average == first) {
        change = 0;
    }
    let klass = change_class(change);
    if (change == Infinity) {
        change = "---------";
    } else {
        change = change.toFixed(2) + '%';
    }
    return `<td class=${klass}>${change}</td>`;
}

function change_class(c) {
    if (c > 1) {
        return 'positive';
    } else if (c > 0.1) {
        return 'slightly-positive';
    } else if (c < -1) {
        return 'negative';
    } else if (c < -0.1) {
        return 'slightly-negative';
    } else {
        return '';
    }
}

function average_by(array, f = (a) => a) {
    return array.reduce((accumulator, current) => accumulator + f(current), 0) / array.length;
}

function max_rss(time) {
    return time.reduce((accumulator, current) => {
        let rss = Math.max(parseFloat(current.after_rss), parseFloat(current.before_rss));
        return Math.max(accumulator, rss);
    }, 0);
}

function bench_detail(bench) {
    let data = bench.builds.map(build => {
        let entries = {};

        for (const instance of build.times) {
            for (const data of instance) {
                if (entries[data.name] === undefined) {
                    entries[data.name] = [];
                }
                entries[data.name].push(data);
            }
        }

        let entries_avg = {};

        for (const entry in entries) {
            entries_avg[entry] = {
                time: average_by(entries[entry], entry => entry.time),
                rss: average_by(entries[entry], entry => parseFloat(entry.after_rss) * 1024 * 1024),
            };
        }

        return entries_avg;
    });

    let times = bench.builds[0].times[0].map(entry => entry.name).filter(entry => {
        return data.every(build => {
            return build[entry] != undefined;
        });
    });

    let table = {
        type: 'Stage',
        columns: [{ name: 'Time', format: format_time }, { name: 'Memory', format: format_size }],
        rows: times.map(event => {
            return {
                name: event,
                columns: [data.map(build => build[event].time), data.map(build => build[event].rss)]
            };
        })
    };

    return `<div><h3 id="${bench.name}">Details of <b>${format_bench(bench.name)}</b></h3>${diff_table(table)}</div>`;
}

function escapeHTML(str) {
    var p = document.createElement("p");
    p.innerText = str
    return p.innerHTML;
}

function build_details() {
    const BENIGN_OPTS = ['changelog-seen', 'rust.deny-warnings', 'rust.deny-warnings', 'build.low-priority'];

    function linearize(keys, path, out) {
        for (const key in keys) {
            let new_path = path.concat([key]);
            if (typeof keys[key] === 'object') {
                linearize(keys[key], new_path, out)
            } else {
                if (!BENIGN_OPTS.includes(new_path.join('.'))) {
                    out.push({ type: 'x.py', key: new_path.join('.'), value: JSON.stringify(keys[key]) });
                }
            }
        }
    }

    for (let i = 0; i < DATA.builds.length; i++) {
        const build = DATA.builds[i];
        let out = [];
        linearize(build.config, [], out);
        const config = DATA.build_configs[i];
        out = out.concat(config.rflags.map(f => { return { type: 'rustc', key: f }; }));
        out = out.concat(config.cflags.map(f => { return { type: 'cargo', key: f }; }));
        out = out.concat(config.envs.map(e => { return { type: 'env', key: e[0], value: e[1] }; }));
        if (config.threads) {
            out.push({ type: 'threads' });
        }
        build.bench_config = out;
    }

    let common_opts = DATA.builds[0].bench_config.filter(opt => {
        return DATA.builds.every(build => {
            return build.bench_config.find(build_opt => JSON.stringify(opt) == JSON.stringify(build_opt)) !== undefined
        });
    });

    if (DATA.builds.length == 1) {
        common_opts = []
    }

    let result = `<div class="build-container">`;

    function render_opt(opt) {
        let result = ``;
        if (opt.type == 'x.py') {
            result += `<div class="split"><p>${opt.key}:</p><p><b>${escapeHTML(opt.value)}</b></p></div>`;
        }
        if (opt.type == 'env') {
            result += `<div class="split"><p><span class="bench-opt">env</span>${escapeHTML(opt.key)}:</p><p><b>${escapeHTML(opt.value)}</b></p></div>`;
        }
        if (opt.type == 'rustc') {
            result += `<div class="split"><p><span class="bench-opt">rustc</span></p><p><b>${escapeHTML(opt.key)}</b></p></div>`;
        }
        if (opt.type == 'cargo') {
            result += `<div class="split"><p><span class="bench-opt">cargo</span></p><p><b>${escapeHTML(opt.key)}</b></p></div>`;
        }
        if (opt.type == 'threads') {
            result += `<p class="l">Multithreaded cargo</p>`;
        }
        return result;
    }

    if (common_opts.length > 0) {
        result += `<div class="common-build">`;
        let build_opts = common_opts.filter(opt => opt.type == 'x.py');
        if (build_opts.length > 0) {
            result += `<div class="build"><h3>Common build options</h3>`;
            for (const opt of build_opts) {
                result += render_opt(opt);
            }
            result += `</div>`;
        }
        let bench_opts = common_opts.filter(opt => opt.type != 'x.py');
        if (bench_opts.length > 0) {
            result += `<div class="build"><h3>Common bench options</h3>`;
            for (const opt of bench_opts) {
                result += render_opt(opt);
            }
            result += `</div>`;
        }
        result += `</div>`;
    }

    for (let i = 0; i < DATA.builds.length; i++) {
        let build = DATA.builds[i];
        result += `<div class="build"><h3>Build <b>${build.name}</b></h3>`;
        result += `<div class="split"><p>From repo:</p><p><b>${build.repo}</b> at ${build.repo_path}</p></div>`;

        result += `<div class="split"><p>Git commit title:</p><p><b>${build.commit_title}</b></p></div>`;
        result += `<div class="split"><p>Git commit:</p><p><b>${build.commit_short}</b></p></div>`;
        result += `<div class="split"><p>Git branch:</p><p><b>${build.branch}</b></p></div>`;

        result += `<div class="split"><p>Upstream commit:</p><p><b>${build.upstream_short}</b></p></div>`;
        if (DATA.builds.length > 1) {
            if (!DATA.builds.find(b => b.commit == build.upstream)) {
                result += `<p class="extra-opts">Not comparing against upstream commit</p>`;
            }
        }

        result += `<div class="split"><p>Triple:</p><p><b>${build.triple}</b></p></div>`;

        let opts = build.bench_config.filter(opt => common_opts.find(common_opt => JSON.stringify(opt) == JSON.stringify(common_opt)) === undefined);

        if (opts.length > 0) {
            let build_opts = opts.filter(opt => opt.type == 'x.py');
            if (build_opts.length > 0) {
                if (DATA.builds.length > 1) {
                    result += `<div class="extra-opts"><h4>Additional build options:</h4>`;
                } else {
                    result += `<div><h4>Build options:</h4>`;
                }

                for (const opt of build_opts) {
                    result += render_opt(opt);
                }
                result += `</div>`;
            }

            let bench_opts = opts.filter(opt => opt.type != 'x.py');
            if (bench_opts.length > 0) {
                if (DATA.builds.length > 1) {
                    result += `<div class="extra-opts"><h4>Additional bench options:</h4>`;
                } else {
                    result += `<div><h4>Bench options:</h4>`;
                }

                for (const opt of bench_opts) {
                    result += render_opt(opt);
                }
                result += `</div>`;
            }
        }

        result += `</div>`;
    }

    result += `</div>`;

    return result;
}

function diff_table(data) {
    let result = `<table><tr><th rowspan="2">${data.type}</th>`;

    for (const column of data.columns) {
        for (let i = 0; i < DATA.builds.length; i++) {
            const build = DATA.builds[i];
            result += `<th colspan="${i > 0 ? 2 : 1}" class="bh">${build.name}</th>`
        }
    }

    result += "</tr><tr>";

    for (const column of data.columns) {
        for (let i = 0; i < DATA.builds.length; i++) {
            result += `<th class="r">${column.name}</th>`;
            if (i > 0) {
                result += `<th class="r">%</th>`;
            }
        }
    }

    result += "</tr>";

    for (const row of data.rows) {
        result += `<tr><th>${row.name}</th>`;

        for (let i = 0; i < row.columns.length; i++) {
            const column = row.columns[i];
            let first = column[0];
            let format = data.columns[i].format;

            for (let j = 0; j < DATA.builds.length; j++) {
                result += `<td>${format(column[j])}</td>`
                if (j > 0) {
                    let change = (column[j] / first - 1) * 100;
                    result += `<td class=${change_class(change)}> ${change.toFixed(2)}%</td>`;
                }
            }
        }
        result += "</tr>";
    }
    result += "</table>";

    return result;
}

function export_bench_times() {
    let benchs = Object.fromEntries(DATA.benchs.map(bench => {
        let times = bench.builds.flatMap(build => build.time);
        return [bench.name, average_by(times)];
    }));
    console.log(benchs);
    let base = average_by(['syntex_syntax:check', 'regex:check', 'hyper:check'].map(b => benchs[b]));
    let result = "";
    for (let bench in benchs) {
        console.log(bench, benchs[bench], base);
        result += `${JSON.stringify(bench)} = ${JSON.stringify(benchs[bench] / base)}\n`;
    }
    copy(result);
    console.log(result);
    return "Copied to clipboard";
}

function summary() {
    let summary = {
        type: 'Benchmark',
        columns: [{ name: 'Time', format: format_time }],
        rows: DATA.benchs.map(bench => {
            let times = bench.builds.map(build => average_by(build.time));
            let rss;
            if (DETAILS) {
                rss = bench.builds.map(build => average_by(build.times, time => max_rss(time) * 1024 * 1024))
            };
            let name = DETAILS ? `<a href="#${bench.name}">${format_bench(bench.name)}</a>` : format_bench(bench.name);
            return { name: name, columns: DETAILS ? [times, rss] : [times] };
        })
    };

    if (DETAILS) {
        summary.columns.push({ name: 'Memory', format: format_size });
    }

    let times = DATA.benchs.map(bench => {
        let first = average_by(bench.builds[0].time);
        let times = bench.builds.map(build => average_by(build.time) / first);

        let rss;
        if (DETAILS) {
            let first_rss = average_by(bench.builds[0].times, time => max_rss(time) * 1024 * 1024);
            rss = bench.builds.map(build => average_by(build.times, time => max_rss(time) * 1024 * 1024) / first_rss);
        } else {
            rss = bench.builds.map(build => 0);
        }
        return { time: times, rss: rss };
    });

    let times_r = times.reduce((sum, v) => {
        return sum.map((sum, i) => {
            return { time: sum.time + v.time[i], rss: sum.rss + v.rss[i] };
        });
    }, DATA.benchs[0].builds.map(bench => { return { time: 0, rss: 0 }; }));

    let times_a = times_r.map(build => {
        return { time: build.time / DATA.benchs.length, rss: build.rss / DATA.benchs.length };
    });

    if (DETAILS) {
        summary.rows.push({
            name: `Summary`, columns: [times_a.map(build => build.time), times_a.map(build => build.rss * 1024 * 1024)]
        });
    } else {
        summary.rows.push({
            name: `Summary`, columns: [times_a.map(build => build.time)]
        });
    }
    return `<div><h3>Benchmark summary</h3>${diff_table(summary)}</div>`;
}

const build_sizes = (() => {
    let dbg_filter = file => !file.path.endsWith(".pdb");

    let std_sizes = DATA.builds.map(build => {
        return build.files.filter(dbg_filter)
            .filter(file => file.path.replaceAll("\\", "/").startsWith(`lib/rustlib/${build.triple}/lib/`))
            .reduce((a, b) => a + b.size, 0);
    });

    let compiler_sizes = DATA.builds.map(build => {
        return build.files.filter(dbg_filter)
            .filter(file => file.path.replaceAll("\\", "/").startsWith("bin/"))
            .reduce((a, b) => a + b.size, 0);
    });

    let total_sizes = DATA.builds.map(build => {
        return build.files.filter(dbg_filter).reduce((a, b) => a + b.size, 0);
    });

    let total_with_dbg_sizes = DATA.builds.map(build => {
        return build.files.reduce((a, b) => a + b.size, 0);
    });

    let size = {
        type: '',
        columns: [{ name: 'Size', format: format_size }],
        rows: [
            { name: 'Compiler size', columns: [compiler_sizes] },
            { name: 'Std size', columns: [std_sizes] },
            { name: 'Total size', columns: [total_sizes] },
            { name: 'Total with debug info', columns: [total_with_dbg_sizes] }
        ]
    };

    return `<div><h3>Build size summary</h3>${diff_table(size)}</div>`;
})();

let file_map = {};

for (const build of DATA.builds) {
    for (const file of build.files) {
        file_map[file.path] = true;
    }
}

let files = Object.keys(file_map).sort();

let compiler_sizes = DATA.builds.map(build => {
    return build.files
        .filter(file => file.path.replaceAll("\\", "/").startsWith("bin/"))
        .reduce((a, b) => a + b.size, 0);
});

let file_size = {
    type: 'File',
    columns: [{ name: 'Size', format: format_size }],
    rows: files.map(file => {
        return {
            name: file, columns: [DATA.builds.map(build => {
                let f = build.files.find(f => f.path === file);
                return f == undefined ? 0 : f.size;
            })]
        };
    })
};

const file_sizes = `<div><h3>Build size details</h3>${diff_table(file_size)}</div>`;

let title = `Benchmark results for `;

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    if (i > 0) {
        title += ` vs. `;
    }
    title += `<b>${DATA.benchs[0].builds[i].build}</b>`;
}

let content = `<div>`;
content += `<h1>${title}</h1><p>Results are the average of ${DATA.benchs[0].builds[0].time.length} execution(s).</p>`;
content += `<div class="flex">`;
content += summary();
content += build_sizes;
content += `</div>`;
content += build_details();
content += `<div class="flex">`;
content += file_sizes;
if (DETAILS) {
    for (const bench of DATA.benchs) {
        content += bench_detail(bench);
    }
}
content += `</div>`;
content += `</div>`;
document.body.innerHTML = content;
