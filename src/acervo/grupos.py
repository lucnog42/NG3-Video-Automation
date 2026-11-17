#!/usr/bin/env python3
"""Grupos: mídias de uma pasta ou de pastas diferentes que viram um projeto.

Um grupo junta mídias do catálogo (de uma subpasta inteira ou de uma seleção que mistura
pastas e importações) e aparece na lista de projetos, com o próprio campo de roteiro: a IA e
a busca de falas e b-roll consideram só o que está no grupo (`plano/roteiros.py`, `grupo=`).

  * é anotação: `grupo` e `grupo_midia` no `midia.sqlite`; nada é copiado nem movido;
  * a mesma mídia pode estar em vários grupos;
  * convive com o "projeto de uma pasta" (`projetos/pasta.py`), que monta o `.fcproj` direto;
  * mudar as mídias depois não regera nada: o roteiro do grupo avisa o que mudou.

    python3 src/acervo/grupos.py listar
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import json
import os
import time

from acervo import biblioteca  # noqa: E402
from nucleo import config  # noqa: E402


def _abs(caminho: str) -> str:
    return caminho if os.path.isabs(caminho) else os.path.join(config.carregar()["acervo"]["raiz"], caminho)


def ids_da_pasta(subpasta: str, fonte: str = "", db: str | None = None) -> list[int]:
    """As mídias de uma subpasta (e das de dentro dela), opcionalmente só de uma fonte."""
    con = biblioteca.abrir(db)
    sp = subpasta.strip().rstrip("/")
    onde, args = ["ausente = 0", "(subpasta = ? or subpasta like ?)"], [sp, sp + "/%"]
    if fonte:
        onde.append("fonte_id = ?")
        args.append(fonte)
    out = [r[0] for r in con.execute(f"select id from midia where {' and '.join(onde)} order by caminho", args)]
    con.close()
    return out


def criar(nome: str, ids: list[int] | None = None, subpasta: str = "", fonte: str = "",
          db: str | None = None) -> dict:
    """Cria o grupo com as mídias de `ids` e/ou de uma subpasta inteira."""
    nome = (nome or "").strip()
    alvo = [int(i) for i in ids or []]
    if subpasta:
        alvo += ids_da_pasta(subpasta, fonte, db)
    alvo = list(dict.fromkeys(alvo))
    if not alvo:
        raise ValueError("o grupo precisa de ao menos uma mídia")
    con = biblioteca.abrir(db)
    if not nome:
        nome = os.path.basename(subpasta.rstrip("/")) if subpasta else f"Grupo {time.strftime('%d/%m %H:%M')}"
    gid = con.execute("insert into grupo (nome, criado_em) values (?, ?)", (nome, time.time())).lastrowid
    con.executemany("insert or ignore into grupo_midia (grupo_id, midia_id) values (?, ?)", [(gid, i) for i in alvo])
    con.commit()
    con.close()
    return ler(gid, db)


def _resumo(con, g) -> dict:
    n = {r["tipo"]: r["n"] for r in con.execute(
        "select m.tipo, count(*) n from grupo_midia gm join midia m on m.id = gm.midia_id "
        "where gm.grupo_id = ? and m.ausente = 0 group by m.tipo", (g["id"],))}
    pastas = [r[0] for r in con.execute(
        "select distinct m.subpasta from grupo_midia gm join midia m on m.id = gm.midia_id where gm.grupo_id = ? "
        "order by m.subpasta limit 6", (g["id"],))]
    return {"id": g["id"], "nome": g["nome"], "criado_em": g["criado_em"], "nota": g["nota"] or "",
            "videos": n.get("video", 0), "imagens": n.get("imagem", 0), "n": sum(n.values()),
            "pastas": [p or "(raiz)" for p in pastas]}


def ler(gid: int, db: str | None = None) -> dict:
    con = biblioteca.abrir(db)
    g = con.execute("select * from grupo where id = ?", (int(gid),)).fetchone()
    if not g:
        con.close()
        raise ValueError(f"grupo inexistente: {gid}")
    out = _resumo(con, g)
    out["ids"] = [r[0] for r in con.execute("select midia_id from grupo_midia where grupo_id = ?", (int(gid),))]
    con.close()
    return out


def listar(db: str | None = None) -> list[dict]:
    con = biblioteca.abrir(db)
    out = [_resumo(con, g) for g in con.execute("select * from grupo order by criado_em desc")]
    con.close()
    return out


def adicionar(gid: int, ids: list[int], db: str | None = None) -> int:
    ler(gid, db)
    con = biblioteca.abrir(db)
    n = con.executemany("insert or ignore into grupo_midia (grupo_id, midia_id) values (?, ?)",
                        [(int(gid), int(i)) for i in ids]).rowcount
    con.commit()
    con.close()
    return n


def tirar(gid: int, ids: list[int], db: str | None = None) -> int:
    con = biblioteca.abrir(db)
    n = con.executemany("delete from grupo_midia where grupo_id = ? and midia_id = ?",
                        [(int(gid), int(i)) for i in ids]).rowcount
    con.commit()
    con.close()
    return n


def renomear(gid: int, nome: str, nota: str | None = None, db: str | None = None) -> dict:
    if not (nome or "").strip():
        raise ValueError("nome vazio")
    con = biblioteca.abrir(db)
    con.execute("update grupo set nome = ? where id = ?", (nome.strip(), int(gid)))
    if nota is not None:
        con.execute("update grupo set nota = ? where id = ?", (nota, int(gid)))
    con.commit()
    con.close()
    return ler(gid, db)


def apagar(gid: int, db: str | None = None) -> None:
    """Apaga só o grupo (a anotação). As mídias e os roteiros já escritos ficam."""
    con = biblioteca.abrir(db)
    con.execute("delete from grupo_midia where grupo_id = ?", (int(gid),))
    con.execute("delete from grupo where id = ?", (int(gid),))
    con.commit()
    con.close()


def arquivos(gid: int, tipo: str = "", db: str | None = None) -> list[str]:
    """Caminhos absolutos das mídias do grupo (só vídeos, com `tipo="video"`)."""
    con = biblioteca.abrir(db)
    extra, args = ("and m.tipo = ?", [int(gid), tipo]) if tipo else ("", [int(gid)])
    out = [_abs(r[0]) for r in con.execute(
        f"select m.caminho from grupo_midia gm join midia m on m.id = gm.midia_id "
        f"where gm.grupo_id = ? and m.ausente = 0 {extra} order by m.caminho", args)]
    con.close()
    return out


def main() -> None:
    print(json.dumps(listar(), ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
