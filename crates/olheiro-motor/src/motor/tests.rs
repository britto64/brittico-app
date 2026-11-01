use super::*;

fn esperar(m: &Motor, segundos: f64, ate: impl Fn(&Estado) -> bool) -> Estado {
    let fim = Instant::now() + Duration::from_secs_f64(segundos);
    loop {
        let e = m.estado();
        if ate(&e) || Instant::now() > fim {
            return e;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn pasta_de_teste(nome: &str) {
    let d = std::env::temp_dir().join(format!("olheiro-motor-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::env::set_var("OLHEIRO_DIR", &d);
    // endereço vazio de propósito: o padrão é o servidor de produção, e o teste não pode entrar nele
    std::env::set_var("OLHEIRO_PORTEIRO", "");
}

/// Uma partida simulada inteira, sem servidor de voz: o motor acha a partida, sabe quem é você e onde os aliados estão.
#[test]
fn a_partida_simulada_vira_partida_com_posicoes_e_sem_sala() {
    pasta_de_teste("simulada");
    let m = Motor::iniciar(|| {});
    m.mandar(Comando::Simular(Some(2)));
    let e = esperar(&m, 20.0, |e| e.posicoes.len() >= 4);
    let p = e.partida.as_ref().expect("a partida simulada foi achada");
    assert_eq!(e.fase, Fase::EmPartida);
    assert_eq!(p.voce, "ana#br1", "a vaga 2 do time fixo é a Ana");
    assert_eq!(p.jogadores.len(), 10);
    assert_eq!(p.meu_time, "ORDER");
    assert!(e.posicoes.len() >= 4, "{:?}", e.posicoes);
    assert!(e.posicoes.keys().all(|k| p.jogador(k).is_some_and(|j| j.time == "ORDER")), "só aliados são lidos");
    assert!(m.icone(&p.jogador("ana#br1").unwrap().alias).is_some(), "a tela tem o ícone");
    // sem porteiro não se tenta sala nenhuma
    assert_eq!(e.sala.conexao, Conexao::Fora);
    m.mandar(Comando::Desligar);
    let e = esperar(&m, 3.0, |e| e.partida.is_none());
    assert!(!e.ligado && e.partida.is_none() && e.posicoes.is_empty());
}
