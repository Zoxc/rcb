console.log(DATA);

let summary = "<table><tr><th>Benchmark</th>";

for (const config of DATA.benchs[0].builds) {
    summary += `<th>${config.build}</th>`
}

summary += "</tr>";

for (const bench of DATA.benchs) {

    summary += `<tr><th>${bench.name}</th>`;
    for (const config of bench.builds) {

        summary += `<td>${config.build}</td>`
    }
    summary += "</tr>";

}

summary += "</table>";

document.body.innerHTML = `<div><h1>${document.title}</h1>${summary}</div>`